//! Die produktiven Navigations-Tools des MCP-Readers (Pool::LocalNavigation).
//!
//! Jedes Tool ist eine dünne Hülle um genau ein AKN-Primitiv aus `fedlex-akn`,
//! gespeist durch den [`AknFetcher`] aus `fedlex-bridge` (Direct Fetch gegen
//! Fedlex, gecached pro Manifestations-URL). Den Stichtag liest NIE ein Tool
//! selbst aus seinen Argumenten — er kommt immer aus dem [`ToolContext`]-Stempel,
//! den der Transport zentral setzt (`params.as_of` > `arguments.as_of`,
//! ADR-011) — so können Tools den Stichtag nicht untereinander verfälschen.
//!
//! Provenance (ADR-004). Wo das Primitiv selbst ein [`Response`] liefert
//! (TXT-01/02/03, STR-01), gilt dessen Herkunft, denn sie stammt strukturell
//! aus dem FRBR-Block des geparsten Dokuments — das Stand-Datum der von der
//! Bridge aufgelösten Fassung wird nachgetragen ([`with_fassung`]). Wo das
//! Primitiv nackt ist (TXT-04, DOC-02/03), gilt die Herkunft der
//! JOLux-Auflösung der Bridge direkt.

use crate::tool::{McpTool, ToolContext, ToolError, ToolPool};
use async_trait::async_trait;
use fedlex_akn::{
    AknDocument, classify_pattern, detect_foreign_content, extract_change_notes, extract_tables,
    get_all_references, get_article_text, get_document_structure, get_element_text,
    get_frbr_metadata, get_modifications, get_readable_document, list_components,
    parse_unlinked_ref, search_text,
};

use fedlex_bridge::{AknFetcher, BridgeError, XmlSource};
use fedlex_core::{Eli, Provenance, Response, ValidAsOf};
use fedlex_jolux::{JoluxError, Language, SparqlClient};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::registry::Registry;

/// Registriert alle produktiven Navigations-Tools an der Registry.
///
/// Ein gemeinsamer Fetcher für alle Tools, damit sie denselben
/// Manifestations-Cache teilen (ein Erlass wird pro Fassung genau einmal
/// geholt, egal welches Tool zuerst fragt).
pub fn register_navigation_tools<C, S>(registry: &mut Registry, fetcher: Arc<AknFetcher<C, S>>)
where
    C: SparqlClient + Send + Sync + 'static,
    S: XmlSource + Send + Sync + 'static,
{
    registry.register(Arc::new(ReadArticle {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(ReadElement {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(GetStructure {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(SearchText {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(GetMetadata {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(GetReferences {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(GetModifications {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(CompareVersions {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(ExtractTables {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(ListComponents {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(DetectForeignContent {
        fetcher: Arc::clone(&fetcher),
    }));
    // G-2-Rest, projiziert 2026-07-02 (ADR-010).
    registry.register(Arc::new(ExtractChangeNotes {
        fetcher: Arc::clone(&fetcher),
    }));
    registry.register(Arc::new(ParseUnlinkedRef));
    registry.register(Arc::new(ReadDocument { fetcher }));
}

// ---------------------------------------------------------------------------
// Argument-Parsing & Fehler-Mapping
// ---------------------------------------------------------------------------

/// Pflicht-Argument `eli` lesen und validieren.
fn arg_eli(args: &Value) -> Result<Eli, ToolError> {
    // 68 §F-12: tolerant gegen volle fedlex-URLs und chunk_ids (zentral).
    crate::tool::require_eli(args)
}

/// Pflicht-Argument mit gegebenem Namen als String lesen (68 §F-16:
/// unterscheidet «fehlt» von «falscher Typ», zentral in [`crate::tool`]).
use crate::tool::require_str as arg_str;

/// Optionales Argument `lang` lesen (Default Deutsch).
fn arg_lang(args: &Value) -> Result<Language, ToolError> {
    // 68 §F-16/F-22: falscher Typ fiel still auf Deutsch zurueck.
    if let Some(v) = args.get("lang")
        && !v.is_null()
        && !v.is_string()
    {
        return Err(ToolError::InvalidArguments(format!(
            "`lang` muss ein String (de|fr|it|en|rm) sein, nicht {}",
            crate::tool::json_type_name(v)
        )));
    }
    match args.get("lang").and_then(Value::as_str) {
        None => Ok(Language::De),
        Some("de") => Ok(Language::De),
        Some("fr") => Ok(Language::Fr),
        Some("it") => Ok(Language::It),
        Some("en") => Ok(Language::En),
        Some("rm") | Some("roh") => Ok(Language::Roh),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "`lang` muss de|fr|it|en|rm sein, nicht `{other}`"
        ))),
    }
}

/// Bridge-Fehler in lenkende Tool-Fehler übersetzen.
fn map_bridge(err: BridgeError) -> ToolError {
    match err {
        BridgeError::Jolux(JoluxError::NotFound(what)) => ToolError::NotFound(what),
        // Verify-L7: 4xx vom SPARQL-Endpoint = permanenter Eingabefehler.
        BridgeError::Jolux(JoluxError::BadRequest(_)) => {
            ToolError::InvalidArguments(err.to_string())
        }
        other => ToolError::Upstream(other.to_string()),
    }
}

/// AKN-Primitiv-Fehler in lenkende Tool-Fehler übersetzen.
fn map_akn(err: fedlex_akn::AknError) -> ToolError {
    use fedlex_akn::AknError;
    match err {
        AknError::EidNotFound(eid) => ToolError::NotFound(format!("eId `{eid}`")),
        AknError::WrongElementKind { .. } | AknError::ComponentNotFound { .. } => {
            ToolError::InvalidArguments(err.to_string())
        }
        other => ToolError::Upstream(other.to_string()),
    }
}

/// Serialisierung eines bereits validierten Domänen-Typs.
fn to_value<T: serde::Serialize>(data: T) -> Result<Value, ToolError> {
    serde_json::to_value(data).map_err(|e| ToolError::Upstream(format!("serialize: {e}")))
}

/// Trägt das Stand-Datum der von der Bridge aufgelösten Fassung
/// (`date_applicability`) in eine AKN-seitig gebaute Provenance nach.
/// Erst damit sieht der Konsument, welche Konsolidierung den Text wirklich
/// trägt — der Stichtag allein suggeriert sonst (etwa bei künftigen
/// Stichtagen) eine bestätigte Fassung, die es nicht gibt.
fn with_fassung(mut prov: Provenance, doc_prov: &Provenance) -> Provenance {
    prov.date_applicability = doc_prov.date_applicability.clone();
    prov
}

/// Gemeinsamer erster Schritt aller Tools. Erlass zum Stichtag beschaffen.
async fn fetch<C, S>(
    fetcher: &AknFetcher<C, S>,
    ctx: &ToolContext,
    args: &Value,
) -> Result<Response<Arc<AknDocument>>, ToolError>
where
    C: SparqlClient + Send + Sync,
    // `'static` wegen des Single-Flight-Caches (67 §H-5): die geteilte
    // XML-Quelle wandert in den Init-Future des Cache-Eintrags.
    S: XmlSource + Send + Sync + 'static,
{
    let eli = arg_eli(args)?;
    let lang = arg_lang(args)?;
    let as_of = ctx.stamp.valid_as_of();
    fetcher
        .fetch_akn_document(&eli, as_of, lang)
        .await
        .map_err(|err| match err {
            // 68 §F-2/F-13: Das nackte «not found: <uri>» wurde als «Erlass
            // existiert nicht» gelesen — real fehlte nur die XML-Fassung zum
            // Stichtag (aeltere Konsolidierungen gibt es erst ab ~2021 als
            // XML, J14.2) oder die Sprachfassung (rm). Die Meldung benennt
            // jetzt, WAS fehlt; der Hint (tool.rs) nennt die Prüfwege.
            BridgeError::Jolux(JoluxError::NotFound(_)) => ToolError::NotFound(format!(
                "keine konsolidierte XML-Fassung fuer `{}` zum Stichtag {} in Sprache `{}`",
                eli.as_str(),
                as_of,
                lang.tag()
            )),
            other => map_bridge(other),
        })
}

// ---------------------------------------------------------------------------
// Die Tools
// ---------------------------------------------------------------------------

/// AKN-TXT-01. Volltext eines Artikels (erzwingt `<article>`).
struct ReadArticle<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for ReadArticle<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "read_article"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Volltext eines Artikels eines Erlasses zum Stichtag (AKN-TXT-01).",
            "properties": {
                "eli": { "type": "string", "description": "Work-ELI, z.B. eli/cc/2017/762" },
                "eid": { "type": "string", "description": "Artikel-eId, z.B. art_1" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli", "eid"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let doc = fetch(&self.fetcher, ctx, &args).await?;
        let eid = arg_str(&args, "eid")?;
        let (text, prov) = get_article_text(doc.data(), eid, ctx.stamp.valid_as_of())
            .map_err(map_akn)?
            .into_parts();
        Ok(Response::new(
            to_value(text)?,
            with_fassung(prov, doc.provenance()),
        ))
    }
}

/// AKN-TXT-02. Volltext eines beliebigen eId-Elements (level, chapter, ...).
struct ReadElement<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for ReadElement<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "read_element"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Volltext eines beliebigen Gliederungselements (AKN-TXT-02). Für LEVEL_BASED-Erlasse ohne Artikel.",
            "properties": {
                "eli": { "type": "string" },
                "eid": { "type": "string", "description": "eId des Elements, z.B. lvl_1 oder chap_2" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli", "eid"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let doc = fetch(&self.fetcher, ctx, &args).await?;
        let eid = arg_str(&args, "eid")?;
        let (text, prov) = get_element_text(doc.data(), eid, ctx.stamp.valid_as_of())
            .map_err(map_akn)?
            .into_parts();
        Ok(Response::new(
            to_value(text)?,
            with_fassung(prov, doc.provenance()),
        ))
    }
}

/// AKN-STR-01. Gliederung des Erlasses, optional auf einen Element-Typ geflacht.
struct GetStructure<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for GetStructure<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "get_structure"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Inhaltsverzeichnis des Erlasses (AKN-STR-01). Default ist das Skelett bis Artikel-Ebene (Orientierungsansicht); depth=full liefert den kompletten Baum bis auf Absatz-Ebene. type_filter=article liefert die flache Artikel-Liste.",
            "properties": {
                "eli": { "type": "string" },
                "type_filter": { "type": "string", "description": "Optional, z.B. article oder chapter" },
                "depth": { "type": "string", "enum": ["article", "full"], "default": "article", "description": "article: Unterelemente von Artikeln gekappt (kompakt). full: kompletter Baum." },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let doc = fetch(&self.fetcher, ctx, &args).await?;
        let filter = args.get("type_filter").and_then(Value::as_str);
        let (mut outline, prov) =
            get_document_structure(doc.data(), filter, ctx.stamp.valid_as_of())
                .map_err(map_akn)?
                .into_parts();
        // 68 §B-1: Das Inhaltsverzeichnis ist ein Orientierungs-Tool — der
        // volle Baum bis auf Absatz-Ebene wog live 95 KB (~24k Tokens) und
        // erschlug genau den Kontext, dem er Überblick geben soll. Default
        // daher Artikel-Skelett; der volle Baum bleibt per depth=full.
        // 68 §F-22: unbekannte depth-Werte fielen still auf den Default —
        // «depth: unsinn» sah aus wie eine beantwortete Wahl.
        let depth = match args.get("depth") {
            None | Some(Value::Null) => "article",
            Some(Value::String(s)) if s == "article" || s == "full" => s.as_str(),
            Some(other) => {
                return Err(ToolError::InvalidArguments(format!(
                    "`depth` muss article|full sein, nicht `{other}`"
                )));
            }
        };
        if depth != "full" {
            prune_below_articles(&mut outline);
        }
        Ok(Response::new(
            to_value(outline)?,
            with_fassung(prov, doc.provenance()),
        ))
    }
}

/// Kappt die Gliederung unterhalb der Artikel-Ebene (68 §B-1) — Kapitel,
/// Abschnitte und Artikel bleiben, Absätze/Ziffern innerhalb von Artikeln
/// fallen weg. Auf LEVEL_BASED-Dokumenten ohne Artikel bleibt der Baum
/// unverändert (dort tragen die Level-Überschriften die Semantik).
fn prune_below_articles(nodes: &mut [fedlex_akn::OutlineNode]) {
    for node in nodes.iter_mut() {
        if node.kind == "article" {
            node.children.clear();
        } else {
            prune_below_articles(&mut node.children);
        }
    }
}

/// AKN-TXT-04. Deterministische Volltextsuche über die eId-Blätter.
struct SearchText<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for SearchText<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "search_text"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Case-insensitive Textsuche innerhalb EINES Erlasses (AKN-TXT-04). Liefert {hits, total, truncated} — total zählt alle Fundstellen, auch über max_hits hinaus. Kein Ersatz für semantische Suche.",
            "properties": {
                "eli": { "type": "string" },
                "query": { "type": "string" },
                "max_hits": { "type": "integer", "default": 20, "maximum": 100 },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli", "query"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let resp = fetch(&self.fetcher, ctx, &args).await?;
        let query = arg_str(&args, "query")?;
        let max_hits = args
            .get("max_hits")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .min(100) as usize;
        // 68 §B-2: total zählt alle Fundstellen, truncated macht die Kappung
        // sichtbar — eine exakt volle Trefferliste las sich vorher als
        // „das ist alles".
        let outcome = search_text(resp.data(), query, max_hits);
        let truncated = outcome.total > outcome.hits.len();
        let prov = resp.provenance().clone();
        Ok(Response::new(
            json!({
                "hits": to_value(outcome.hits)?,
                "total": outcome.total,
                "truncated": truncated,
            }),
            prov,
        ))
    }
}

/// AKN-DOC-02/03. FRBR-Selbstauskunft plus Struktur-Muster des Erlasses.
struct GetMetadata<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for GetMetadata<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "get_metadata"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "FRBR-Metadaten und Struktur-Muster des Erlasses (AKN-DOC-02/03). Vor read_article aufrufen: 91 % der OC/FGA haben keine Artikel.",
            "properties": {
                "eli": { "type": "string" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let resp = fetch(&self.fetcher, ctx, &args).await?;
        let frbr = get_frbr_metadata(resp.data()).map_err(map_akn)?;
        let pattern = classify_pattern(resp.data());
        let prov = resp.provenance().clone();
        Ok(Response::new(
            json!({ "frbr": to_value(frbr)?, "pattern": to_value(pattern)? }),
            prov,
        ))
    }
}

/// AKN-TXT-03. Ganzer Erlass als destilliertes Markdown (Anzeige/Kontext).
struct ReadDocument<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for ReadDocument<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "read_document"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Ganzer Erlass als lesbares Markdown (AKN-TXT-03), mit Zeichen-Budget gegen Kontext-Sprengung. Für Zitate read_article/read_element nutzen; für Überblick get_structure.",
            "properties": {
                "eli": { "type": "string" },
                "max_chars": { "type": "integer", "default": 120000, "description": "Zeichen-Budget; darüber wird gekappt (truncated=true, Fortsetzung via offset=next_offset). 0 = unbegrenzt." },
                "offset": { "type": "integer", "default": 0, "description": "Zeichen-Offset für Fortsetzungs-Lektüre (next_offset der vorigen Antwort)" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let doc = fetch(&self.fetcher, ctx, &args).await?;
        let (markdown, prov) = get_readable_document(doc.data(), ctx.stamp.valid_as_of())
            .map_err(map_akn)?
            .into_parts();
        let prov = with_fassung(prov, doc.provenance());
        // 68 §B-1: Ein einziger read_document-Aufruf wog live 210 KB (~50k
        // Tokens) — mehr als die meisten Agenten-Budgets für den ganzen
        // Recherche-Schritt. Zeichen-Budget mit ehrlichem Truncation-Signal
        // (B-2) statt stiller Vollausgabe; Fortsetzung über offset.
        let max_chars = args
            .get("max_chars")
            .and_then(Value::as_u64)
            .unwrap_or(120_000) as usize;
        let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let total_chars = markdown.chars().count();
        let window: String = match max_chars {
            0 => markdown.chars().skip(offset).collect(),
            m => markdown.chars().skip(offset).take(m).collect(),
        };
        let end = offset + window.chars().count();
        let truncated = end < total_chars;
        Ok(Response::new(
            json!({
                "markdown": window,
                "total_chars": total_chars,
                "truncated": truncated,
                "next_offset": if truncated { Value::from(end) } else { Value::Null },
            }),
            prov,
        ))
    }
}

/// AKN-REF-01. Alle Verweise des Erlasses (Body, Präambel, Fussnoten).
struct GetReferences<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for GetReferences<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "get_references"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Verweise (<ref>) des Erlasses (AKN-REF-01) als Liste {references, total, truncated}. Fedlex-hrefs zeigen auf Work-Ebene, Stichtagsauflösung läuft über die Tools selbst.",
            "properties": {
                "eli": { "type": "string" },
                "limit": { "type": "integer", "default": 200, "description": "Max. Anzahl Verweise (Default 200 — bewusst hoeher als die 20/50-Suchlimits: Verweise sind kompakte Tupel und das Tool paginiert); total nennt die Gesamtzahl, truncated signalisiert die Kappung." },
                "offset": { "type": "integer", "default": 0, "description": "Start-Index für Fortsetzung" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let doc = fetch(&self.fetcher, ctx, &args).await?;
        let (refs, prov) = get_all_references(doc.data(), ctx.stamp.valid_as_of())
            .map_err(map_akn)?
            .into_parts();
        let prov = with_fassung(prov, doc.provenance());
        // 68 §B-1/B-2: live 82 KB an Verweisen in einem Rutsch. Listenform
        // mit total/truncated — der Agent sieht, ob er alles hat.
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(200)
            .max(1) as usize;
        let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let total = refs.len();
        let page: Vec<_> = refs.into_iter().skip(offset).take(limit).collect();
        let truncated = offset + page.len() < total;
        Ok(Response::new(
            json!({
                "references": to_value(page)?,
                "total": total,
                "truncated": truncated,
            }),
            prov,
        ))
    }
}

/// AKN-MOD-01. Änderungsblöcke eines Änderungserlasses (OC).
struct GetModifications<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for GetModifications<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "get_modifications"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Änderungsblöcke (<mod>) eines Änderungserlasses mit neuem Wortlaut (AKN-MOD-01). Leer bei Konsolidierungen, dort sind Mods eingearbeitet.",
            "properties": {
                "eli": { "type": "string" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let doc = fetch(&self.fetcher, ctx, &args).await?;
        let (mods, prov) = get_modifications(doc.data(), ctx.stamp.valid_as_of())
            .map_err(map_akn)?
            .into_parts();
        Ok(Response::new(
            to_value(mods)?,
            with_fassung(prov, doc.provenance()),
        ))
    }
}

/// AKN-SPC-01. Tabellen eines Erlasses als strukturierte Einheiten.
struct ExtractTables<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for ExtractTables<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "extract_tables"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Tabellen eines Erlasses als strukturierte Einheiten (AKN-SPC-01). Tarife, Grenzwerte, Zuständigkeits-Matrizen. `within` schränkt auf einen Teilbaum ein; `oversized=true` markiert >100-Zeilen-Tabellen, die nicht als Einheit chunkbar sind.",
            "properties": {
                "eli": { "type": "string" },
                "within": { "type": "string", "description": "Optionale eId, auf deren Teilbaum eingeschränkt wird, z.B. art_5" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let resp = fetch(&self.fetcher, ctx, &args).await?;
        let within = args.get("within").and_then(Value::as_str);
        let tables = extract_tables(resp.data(), within).map_err(map_akn)?;
        let prov = resp.provenance().clone();
        Ok(Response::new(to_value(tables)?, prov))
    }
}

/// AKN-CMP-01. Anhänge/Beilagen eines Erlasses (eigenständige FRBR-Werke).
struct ListComponents<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for ListComponents<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "list_components"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Anhänge/Beilagen eines Erlasses (AKN-CMP-01). Jeder Anhang ist ein eigenständiges FRBR-Werk mit eigener Identität; `is_empty_stub` markiert überspringbare Leer-Anhänge. Komplementär zu list_annexes (JOLux-Sicht, nur Anhänge mit Impacts).",
            "properties": {
                "eli": { "type": "string" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let resp = fetch(&self.fetcher, ctx, &args).await?;
        let components = list_components(resp.data());
        let prov = resp.provenance().clone();
        Ok(Response::new(to_value(components)?, prov))
    }
}

/// AKN-SPC-02. Eingebettete Fremdinhalte (Formeln, Grafiken, OOXML).
struct DetectForeignContent<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for DetectForeignContent<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "detect_foreign_content"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Eingebettete Fremdinhalte eines Erlasses (AKN-SPC-02): SVG-Grafiken, MathML-Formeln, SKOS, OOXML. Selten, aber juristisch relevant (Berechnungsformeln in Verordnungen). Als Einheit behandeln, nie elementweise zerlegen.",
            "properties": {
                "eli": { "type": "string" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let resp = fetch(&self.fetcher, ctx, &args).await?;
        let foreign = detect_foreign_content(resp.data());
        let prov = resp.provenance().clone();
        Ok(Response::new(to_value(foreign)?, prov))
    }
}

/// Versionsvergleich. Zwei Stichtagsfassungen desselben Erlasses, destilliert
/// zu hinzugefügten, entfernten und geänderten Artikeln (Pool Validation).
struct CompareVersions<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

/// Artikel-Texte eines Dokuments als Map `eid → Normtext`.
fn article_texts(doc: &AknDocument, as_of: ValidAsOf) -> BTreeMap<String, String> {
    let Ok(outline) = get_document_structure(doc, Some("article"), as_of) else {
        return BTreeMap::new();
    };
    outline
        .data()
        .iter()
        .filter_map(|n| n.eid.clone())
        .filter_map(|eid| {
            get_element_text(doc, &eid, as_of)
                .ok()
                .map(|r| (eid, r.into_parts().0.text))
        })
        .collect()
}

#[async_trait]
impl<C, S> McpTool for CompareVersions<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "compare_versions"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Validation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Vergleicht die Stichtagsfassung (as_of der Anfrage) mit einer zweiten Fassung (compare_to). Liefert hinzugefügte, entfernte und geänderte Artikel als Markdown-Destillat.",
            "properties": {
                "eli": { "type": "string" },
                "compare_to": { "type": "string", "description": "Zweiter Stichtag, ISO YYYY-MM-DD" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli", "compare_to"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let eli = arg_eli(&args)?;
        let lang = arg_lang(&args)?;
        let compare_raw = arg_str(&args, "compare_to")?;
        let compare_date = time::Date::parse(
            compare_raw,
            time::macros::format_description!("[year]-[month]-[day]"),
        )
        .map_err(|_| ToolError::InvalidArguments("`compare_to` muss ISO YYYY-MM-DD sein".into()))?;

        // Basis-Fassung zum Anfrage-Stichtag, Vergleichs-Fassung zu compare_to.
        // Die Provenance der Antwort gehört zur Basis-Fassung (ctx.stamp).
        let base = self
            .fetcher
            .fetch_akn_document(&eli, ctx.stamp.valid_as_of(), lang)
            .await
            .map_err(map_bridge)?;
        let other = self
            .fetcher
            .fetch_akn_document(&eli, ValidAsOf::new(compare_date), lang)
            .await
            .map_err(map_bridge)?;

        let base_arts = article_texts(base.data(), ctx.stamp.valid_as_of());
        let other_arts = article_texts(other.data(), ValidAsOf::new(compare_date));

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();
        for (eid, text) in &base_arts {
            match other_arts.get(eid) {
                None => added.push(eid.as_str()),
                Some(old) if old != text => changed.push(eid.as_str()),
                Some(_) => {}
            }
        }
        for eid in other_arts.keys() {
            if !base_arts.contains_key(eid) {
                removed.push(eid.as_str());
            }
        }

        let mut md = format!(
            "# Versionsvergleich {}\n\nBasis {} gegen {}\n\n",
            eli.as_str(),
            ctx.stamp.valid_as_of(),
            compare_date
        );
        let section = |md: &mut String, title: &str, eids: &[&str]| {
            md.push_str(&format!("## {} ({})\n", title, eids.len()));
            if eids.is_empty() {
                md.push_str("- keine\n");
            } else {
                for eid in eids {
                    md.push_str(&format!("- {eid}\n"));
                }
            }
            md.push('\n');
        };
        section(&mut md, "Nur in Basis-Fassung", &added);
        section(&mut md, "Geaendert", &changed);
        section(&mut md, "Nur in Vergleichs-Fassung", &removed);

        let prov = base.provenance().clone();
        Ok(Response::new(
            json!({
                "markdown": md,
                "added": added,
                "changed": changed,
                "removed": removed,
                "compare_to": compare_raw,
            }),
            prov,
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests — Mocks aus fedlex-jolux/fedlex-bridge, kein Netzwerk.
// ---------------------------------------------------------------------------

/// AKN-MOD-02. Aenderungsnotizen (redaktionelle Fussnoten) extrahieren.
struct ExtractChangeNotes<C, S> {
    fetcher: Arc<AknFetcher<C, S>>,
}

#[async_trait]
impl<C, S> McpTool for ExtractChangeNotes<C, S>
where
    C: SparqlClient + Send + Sync,
    S: XmlSource + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        "extract_change_notes"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Extrahiert Aenderungsnotizen (redaktionelle Fussnoten zu AS-Aenderungen) eines Erlasses zum Stichtag (AKN-MOD-02), optional auf ein Element begrenzt.",
            "properties": {
                "eli": { "type": "string", "description": "Work-ELI, z.B. eli/cc/2017/762" },
                "within": { "type": "string", "description": "Optionale eId, auf die die Suche begrenzt wird" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let doc = fetch(&self.fetcher, ctx, &args).await?;
        let within = args.get("within").and_then(Value::as_str);
        let (notes, prov) = extract_change_notes(doc.data(), within, ctx.stamp.valid_as_of())
            .map_err(map_akn)?
            .into_parts();
        Ok(Response::new(
            to_value(notes)?,
            with_fassung(prov, doc.provenance()),
        ))
    }
}

/// AKN-REF-02. Unverlinkten Verweis-Text ("Art. 9a", "SR 730.0") parsen.
struct ParseUnlinkedRef;

#[async_trait]
impl McpTool for ParseUnlinkedRef {
    fn name(&self) -> &str {
        "parse_unlinked_ref"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::LocalNavigation
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Parst einen unverlinkten Verweis-Text (AKN-REF-02), z.B. 'Art. 58 Abs. 1 ParlG', strukturiert: article/paragraph/act_abbreviation plus eid_candidate (direkt fuer read_element; das Kuerzel fuer search_law). Reiner Parser, KEIN Beleg: das Ergebnis ist ein HINWEIS (kind=hint) — belege es anschliessend mit read_article/get_metadata.",
            "properties": {
                "label": { "type": "string", "description": "Der Verweis-Text, z.B. 'Artikel 7 Absatz 2'" }
            },
            "required": ["label"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let label = arg_str(&args, "label")?;
        let parsed = parse_unlinked_ref(label);
        // Ein geparster Verweis ist ein Suchkandidat, kein Beleg (ADR-006) —
        // Hinweis-Provenance mit dem Gattungs-Sentinel der Discovery-Tools.
        let eli = Eli::new("eli/cc").map_err(|e| ToolError::Upstream(e.to_string()))?;
        let prov = ctx.stamp.into_hint_provenance(eli);
        Ok(Response::new(to_value(parsed)?, prov))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthResolver, ClaimRecord, Role, StaticAuthResolver};
    use crate::temporal::TemporalResolver;
    use crate::tool::ToolContext;
    use fedlex_bridge::MockXmlSource;
    use fedlex_core::TransactionTime;
    use fedlex_jolux::MockSparqlClient;
    use time::macros::{date, datetime};

    /// Canned-Ergebnis für JLX-TMP-02 (Variablen cons/date/url).
    const CONS_JSON: &str = r#"{
      "head": { "vars": ["cons", "date", "url"] },
      "results": { "bindings": [ {
        "cons": { "type": "uri", "value": "https://fedlex.data.admin.ch/eli/cc/2017/762/20260401" },
        "date": { "type": "literal", "value": "2026-04-01" },
        "url": { "type": "uri", "value": "https://fedlex.data.admin.ch/filestore/x/de/xml" }
      } ] }
    }"#;

    /// Minimales, aber strukturell echtes AKN-Dokument.
    const MINI_ACT: &str = r##"<akomaNtoso xmlns="http://docs.oasis-open.org/legaldocml/ns/akn/3.0">
      <act>
        <meta><identification source="#me">
          <FRBRWork>
            <FRBRuri value="https://fedlex.data.admin.ch/eli/cc/2017/762/20260401"/>
            <FRBRname xml:lang="de" value="Energiegesetz"/>
          </FRBRWork>
          <FRBRExpression><FRBRlanguage language="de"/></FRBRExpression>
        </identification></meta>
        <body>
          <article eId="art_1">
            <num>Art. 1</num>
            <paragraph eId="art_1/para_1"><content><p>Dieses Gesetz bezweckt eine sichere Energieversorgung.</p></content></paragraph>
            <paragraph eId="art_1/para_2"><content><p>Der Bund sorgt fuer einen sparsamen Verbrauch.</p></content></paragraph>
          </article>
        </body>
      </act>
    </akomaNtoso>"##;

    fn registry() -> Registry {
        let fetcher = Arc::new(AknFetcher::new(
            MockSparqlClient::from_json(CONS_JSON),
            MockXmlSource::new(MINI_ACT),
            1024 * 1024,
        ));
        let mut r = Registry::new();
        register_navigation_tools(&mut r, fetcher);
        r
    }

    fn ctx() -> ToolContext {
        ctx_with_role(Role::Reader)
    }

    fn ctx_with_role(role: Role) -> ToolContext {
        let claims = StaticAuthResolver::new()
            .with_credential(
                "c",
                ClaimRecord {
                    tenant: "kanzlei-a".into(),
                    session: "sess-1".into(),
                    role,
                },
            )
            .verify("c")
            .unwrap();
        let stamp = TemporalResolver::new(date!(2026 - 06 - 01))
            .stamp_at(None, TransactionTime::new(datetime!(2026-06-10 09:00 UTC)));
        ToolContext { claims, stamp }
    }

    #[tokio::test]
    async fn all_navigation_tools_are_listed_for_reader() {
        let names: Vec<String> = registry()
            .list_tools(Role::Reader)
            .into_iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        for expected in [
            "get_metadata",
            "get_modifications",
            "get_references",
            "get_structure",
            "read_article",
            "read_document",
            "read_element",
            "search_text",
            "extract_tables",
            "list_components",
            "detect_foreign_content",
        ] {
            assert!(names.contains(&expected.to_string()), "fehlt: {expected}");
        }
        // Validation-Pool bleibt dem Reader verborgen.
        assert!(!names.contains(&"compare_versions".to_string()));
    }

    #[tokio::test]
    async fn extract_tables_returns_empty_list_for_tableless_act() {
        let out = registry()
            .dispatch(
                &ctx(),
                "extract_tables",
                json!({ "eli": "eli/cc/2017/762" }),
            )
            .await;
        assert!(out["data"].as_array().unwrap().is_empty(), "war: {out}");
        assert_eq!(out["provenance"]["eli"], "eli/cc/2017/762");
    }

    #[tokio::test]
    async fn list_components_returns_empty_list_for_componentless_act() {
        let out = registry()
            .dispatch(
                &ctx(),
                "list_components",
                json!({ "eli": "eli/cc/2017/762" }),
            )
            .await;
        assert!(out["data"].as_array().unwrap().is_empty(), "war: {out}");
        assert_eq!(out["provenance"]["eli"], "eli/cc/2017/762");
    }

    #[tokio::test]
    async fn detect_foreign_content_returns_empty_list_for_plain_act() {
        let out = registry()
            .dispatch(
                &ctx(),
                "detect_foreign_content",
                json!({ "eli": "eli/cc/2017/762" }),
            )
            .await;
        assert!(out["data"].as_array().unwrap().is_empty(), "war: {out}");
        assert_eq!(out["provenance"]["eli"], "eli/cc/2017/762");
    }

    #[tokio::test]
    async fn read_article_returns_text_with_structural_provenance() {
        let out = registry()
            .dispatch(
                &ctx(),
                "read_article",
                json!({ "eli": "eli/cc/2017/762", "eid": "art_1" }),
            )
            .await;
        assert!(
            out["data"]["text"]
                .as_str()
                .unwrap()
                .contains("Energieversorgung"),
            "Antwort war: {out}"
        );
        // ADR-004. Herkunft aus dem FRBR-Block des Dokuments selbst.
        assert_eq!(out["provenance"]["eli"], "eli/cc/2017/762");
        assert!(!out["provenance"]["valid_as_of"].is_null());
    }

    #[tokio::test]
    async fn get_structure_lists_articles() {
        let out = registry()
            .dispatch(
                &ctx(),
                "get_structure",
                json!({ "eli": "eli/cc/2017/762", "type_filter": "article" }),
            )
            .await;
        let articles = out["data"].as_array().expect("Array von OutlineNodes");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0]["eid"], "art_1");
    }

    #[tokio::test]
    async fn search_text_finds_hits_and_carries_provenance() {
        let out = registry()
            .dispatch(
                &ctx(),
                "search_text",
                json!({ "eli": "eli/cc/2017/762", "query": "energieversorgung" }),
            )
            .await;
        // 68 §B-2: Listenform {hits, total, truncated} statt nacktem Array.
        let hits = out["data"]["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["eid"], "art_1/para_1");
        assert_eq!(out["data"]["total"], 1);
        assert_eq!(out["data"]["truncated"], false);
        assert_eq!(out["provenance"]["eli"], "eli/cc/2017/762");
    }

    /// 68 §B-2: max_hits kappt sichtbar — total zählt weiter, truncated=true.
    #[tokio::test]
    async fn search_text_signals_truncation() {
        let out = registry()
            .dispatch(
                &ctx(),
                "search_text",
                json!({ "eli": "eli/cc/2017/762", "query": "e", "max_hits": 1 }),
            )
            .await;
        assert_eq!(out["data"]["hits"].as_array().unwrap().len(), 1);
        assert!(out["data"]["total"].as_u64().unwrap() > 1);
        assert_eq!(out["data"]["truncated"], true);
    }

    #[tokio::test]
    async fn get_metadata_reports_frbr_and_pattern() {
        let out = registry()
            .dispatch(&ctx(), "get_metadata", json!({ "eli": "eli/cc/2017/762" }))
            .await;
        assert_eq!(out["data"]["frbr"]["title"], "Energiegesetz");
        assert!(out["data"]["pattern"].is_object() || out["data"]["pattern"].is_string());
    }

    #[tokio::test]
    async fn read_document_renders_markdown() {
        let out = registry()
            .dispatch(&ctx(), "read_document", json!({ "eli": "eli/cc/2017/762" }))
            .await;
        let md = out["data"]["markdown"].as_str().unwrap();
        assert!(md.contains("# Energiegesetz"));
        // 68 §B-1: Unter dem Default-Budget → vollständig, ehrlich markiert.
        assert_eq!(out["data"]["truncated"], false);
        assert!(out["data"]["total_chars"].as_u64().unwrap() > 0);
    }

    /// 68 §B-1/B-2: Das Zeichen-Budget kappt ehrlich (truncated + next_offset)
    /// und die Fortsetzung über offset liefert lückenlos den Rest.
    #[tokio::test]
    async fn read_document_budget_truncates_and_resumes() {
        let reg = registry();
        let full = reg
            .dispatch(&ctx(), "read_document", json!({ "eli": "eli/cc/2017/762" }))
            .await;
        let full_md = full["data"]["markdown"].as_str().unwrap().to_string();
        let total = full["data"]["total_chars"].as_u64().unwrap();

        let first = reg
            .dispatch(
                &ctx(),
                "read_document",
                json!({ "eli": "eli/cc/2017/762", "max_chars": 10 }),
            )
            .await;
        assert_eq!(first["data"]["truncated"], true);
        assert_eq!(first["data"]["total_chars"].as_u64().unwrap(), total);
        let next = first["data"]["next_offset"].as_u64().unwrap();
        assert_eq!(next, 10);

        let rest = reg
            .dispatch(
                &ctx(),
                "read_document",
                json!({ "eli": "eli/cc/2017/762", "offset": next, "max_chars": 0 }),
            )
            .await;
        let stitched = format!(
            "{}{}",
            first["data"]["markdown"].as_str().unwrap(),
            rest["data"]["markdown"].as_str().unwrap()
        );
        assert_eq!(stitched, full_md, "Fortsetzung muss lückenlos anschliessen");
        assert_eq!(rest["data"]["truncated"], false);
    }

    /// 68 §B-1: Default ist das Artikel-Skelett (Absätze gekappt, leere
    /// children-Arrays gar nicht serialisiert); depth=full liefert den Baum.
    #[tokio::test]
    async fn get_structure_default_is_article_skeleton() {
        let reg = registry();
        let skeleton = reg
            .dispatch(&ctx(), "get_structure", json!({ "eli": "eli/cc/2017/762" }))
            .await;
        let top = &skeleton["data"].as_array().unwrap()[0];
        assert_eq!(top["kind"], "article");
        assert!(
            top.get("children").is_none(),
            "Artikel-Kinder müssen gekappt und leere Arrays weggelassen sein: {top}"
        );

        let full = reg
            .dispatch(
                &ctx(),
                "get_structure",
                json!({ "eli": "eli/cc/2017/762", "depth": "full" }),
            )
            .await;
        let top = &full["data"].as_array().unwrap()[0];
        assert!(
            top["children"].as_array().is_some_and(|c| !c.is_empty()),
            "depth=full muss die Absatz-Ebene tragen: {top}"
        );
    }

    #[tokio::test]
    async fn unknown_eid_yields_not_found_hint() {
        let out = registry()
            .dispatch(
                &ctx(),
                "read_article",
                json!({ "eli": "eli/cc/2017/762", "eid": "art_999" }),
            )
            .await;
        assert!(out["error"].as_str().unwrap().contains("not found"));
        // 68 §C-7: Der eId-Hint nennt das konkrete Folge-Tool.
        assert!(out["hint"].as_str().unwrap().contains("get_structure"));
    }

    #[tokio::test]
    async fn get_references_returns_empty_list_for_refless_act() {
        let out = registry()
            .dispatch(
                &ctx(),
                "get_references",
                json!({ "eli": "eli/cc/2017/762" }),
            )
            .await;
        // 68 §B-1/B-2: Listenform mit total/truncated statt nacktem Array.
        assert!(
            out["data"]["references"].as_array().unwrap().is_empty(),
            "war: {out}"
        );
        assert_eq!(out["data"]["total"], 0);
        assert_eq!(out["data"]["truncated"], false);
        assert_eq!(out["provenance"]["eli"], "eli/cc/2017/762");
    }

    #[tokio::test]
    async fn get_modifications_is_empty_on_consolidation() {
        let out = registry()
            .dispatch(
                &ctx(),
                "get_modifications",
                json!({ "eli": "eli/cc/2017/762" }),
            )
            .await;
        assert!(out["data"].as_array().unwrap().is_empty(), "war: {out}");
    }

    #[tokio::test]
    async fn compare_versions_requires_validator_role() {
        let out = registry()
            .dispatch(
                &ctx(),
                "compare_versions",
                json!({ "eli": "eli/cc/2017/762", "compare_to": "2020-01-01" }),
            )
            .await;
        assert!(out["error"].is_string(), "Reader darf nicht: {out}");
    }

    #[tokio::test]
    async fn compare_versions_reports_identical_versions_as_unchanged() {
        // Der Mock liefert für beide Stichtage dasselbe XML, also darf der
        // Vergleich keine Abweichungen melden.
        let out = registry()
            .dispatch(
                &ctx_with_role(Role::Validator),
                "compare_versions",
                json!({ "eli": "eli/cc/2017/762", "compare_to": "2020-01-01" }),
            )
            .await;
        assert!(out["data"]["added"].as_array().unwrap().is_empty());
        assert!(out["data"]["changed"].as_array().unwrap().is_empty());
        assert!(out["data"]["removed"].as_array().unwrap().is_empty());
        assert!(
            out["data"]["markdown"]
                .as_str()
                .unwrap()
                .contains("Versionsvergleich")
        );
        assert_eq!(out["provenance"]["eli"], "eli/cc/2017/762");
    }

    #[tokio::test]
    async fn compare_versions_rejects_bad_date() {
        let out = registry()
            .dispatch(
                &ctx_with_role(Role::Validator),
                "compare_versions",
                json!({ "eli": "eli/cc/2017/762", "compare_to": "irgendwann" }),
            )
            .await;
        assert!(out["error"].as_str().unwrap().contains("invalid arguments"));
    }

    #[tokio::test]
    async fn missing_eli_is_invalid_arguments() {
        let out = registry()
            .dispatch(&ctx(), "read_article", json!({ "eid": "art_1" }))
            .await;
        assert!(out["error"].as_str().unwrap().contains("invalid arguments"));
        assert!(out["error"].as_str().unwrap().contains("fehlt"));
    }

    /// 68 §F-16: `eid: 21` ist ein Typfehler, kein fehlendes Feld — die
    /// Diagnose «fehlt» war faktisch falsch und irritierte den Agenten.
    #[tokio::test]
    async fn wrong_type_is_reported_as_type_error_not_missing() {
        let out = registry()
            .dispatch(
                &ctx(),
                "read_article",
                json!({ "eli": "eli/cc/2017/762", "eid": 21 }),
            )
            .await;
        let err = out["error"].as_str().unwrap();
        assert!(err.contains("muss ein String sein, nicht Zahl"), "{err}");
        assert!(!err.contains("fehlt"), "{err}");
    }

    /// 68 §F-16/F-22: `lang` mit falschem Typ fiel still auf Deutsch zurück.
    #[tokio::test]
    async fn wrong_lang_type_errors_instead_of_silent_default() {
        let out = registry()
            .dispatch(
                &ctx(),
                "read_article",
                json!({ "eli": "eli/cc/2017/762", "eid": "art_1", "lang": 7 }),
            )
            .await;
        let err = out["error"].as_str().unwrap();
        assert!(err.contains("`lang` muss ein String"), "{err}");
    }

    #[tokio::test]
    async fn wrong_element_kind_guides_to_read_element() {
        // art_1/para_1 ist <paragraph>, read_article erzwingt <article>.
        let out = registry()
            .dispatch(
                &ctx(),
                "read_article",
                json!({ "eli": "eli/cc/2017/762", "eid": "art_1/para_1" }),
            )
            .await;
        assert!(
            out["error"]
                .as_str()
                .unwrap()
                .contains("erwartet <article>")
        );
    }

    #[tokio::test]
    async fn not_found_consolidation_propagates_as_not_found() {
        let fetcher = Arc::new(AknFetcher::new(
            MockSparqlClient::from_json(
                r#"{ "head": { "vars": ["cons","date","url"] }, "results": { "bindings": [] } }"#,
            ),
            MockXmlSource::new(""),
            1024 * 1024,
        ));
        let mut r = Registry::new();
        register_navigation_tools(&mut r, fetcher);
        let out = r
            .dispatch(
                &ctx(),
                "read_article",
                json!({ "eli": "eli/cc/1907/233", "eid": "art_1" }),
            )
            .await;
        assert!(out["error"].as_str().unwrap().contains("not found"));
    }

    /// ADR-010-Abnahme: `parse_unlinked_ref` ist ein reiner Parser und
    /// liefert einen HINWEIS (kind=hint), nie einen Beleg.
    #[tokio::test]
    async fn parse_unlinked_ref_yields_hint_not_norm() {
        let r = registry();
        let result = r
            .dispatch(
                &ctx(),
                "parse_unlinked_ref",
                serde_json::json!({ "label": "Art. 9a" }),
            )
            .await;
        assert_eq!(result["provenance"]["kind"], "hint");
        assert!(
            result["data"].is_object() || result["data"].is_string(),
            "geparster Verweis fehlt: {result}"
        );
        // 68 §C-6: strukturierte Zerlegung erreicht die Tool-Antwort.
        assert_eq!(result["data"]["article"], "9a");
        assert_eq!(result["data"]["eid_candidate"], "art_9a");
    }

    /// ADR-010-Abnahme: `extract_change_notes` läuft über den Fetcher und
    /// trägt Norm-Provenance; ein Dokument ohne Änderungsnotizen liefert
    /// eine leere Liste, keinen Fehler.
    #[tokio::test]
    async fn extract_change_notes_empty_is_list_with_norm_provenance() {
        let r = registry();
        let result = r
            .dispatch(
                &ctx(),
                "extract_change_notes",
                serde_json::json!({ "eli": "eli/cc/2017/762" }),
            )
            .await;
        assert_eq!(result["provenance"]["kind"], "norm", "{result}");
        assert!(result["data"].is_array(), "{result}");
    }
}
