//! Die Discovery-Tools des MCP-Readers (Pool::Discovery, ADR-006).
//!
//! Drei dünne Hüllen um die provenance-losen Discovery-Primitive aus
//! `fedlex-jolux` — `search_law` (JLX-RES-02), `resolve_sr_number` (JLX-RES-01)
//! und `find_related_by_topic` (JLX-TAX-02). Sie schliessen die Lücke „Frage
//! ohne ELI": ein Agent gewinnt aus Titel/SR-Nummer/Thema einen Einstiegs-ELI
//! und belegt ihn anschliessend mit den norm-tragenden Navigations-Tools.
//!
//! ## Hinweis-Provenance statt Norm-Provenance (ADR-006)
//!
//! Ein Suchtreffer ist eine **Hypothese**, kein Beleg. Deshalb trägt jede
//! Discovery-Antwort eine **Hinweis-Provenance** (`ProvenanceKind::Hint`), die
//! sich strukturell von einer Norm-Provenance unterscheidet. Der Konsument
//! (`mcp-fedlex-skills`/`ansV`) kann einen Hinweis nicht versehentlich als
//! Beleg verbuchen, weil der Typ es ausweist.
//!
//! Die Hülle der `Response` trägt eine Hinweis-Provenance auf die **Anfrage
//! selbst** (Sentinel-ELI der Suchgattung), damit das Provenance-Gate auch bei
//! null Treffern erfüllt ist, ohne einen Treffer vorzutäuschen. Die Treffer
//! selbst tragen nur ihr `eli` — eine Per-Hit-Provenance wäre N-fach derselbe
//! Stempel (68 §C-5); verbindlich wird ein Kandidat erst durch einen
//! Norm-Beleg (ADR-006).

use crate::registry::Registry;
use crate::tool::{McpTool, ToolContext, ToolError, ToolPool};
use async_trait::async_trait;
use fedlex_core::{Eli, Provenance, Response};
use fedlex_jolux::{
    JoluxError, Language, SparqlClient, find_related_by_topic, resolve_sr_number, search_law,
};
use fedlex_jolux::{
    explore_node, find_treaties, get_consultation_documents, get_consultations, get_treaty_info,
    list_vocabulary, resolve_vocabulary_label,
};
use serde_json::{Value, json};
use std::sync::Arc;

/// Registriert alle Discovery-Tools an der Registry.
///
/// Sie teilen sich einen SPARQL-Client (Live-Auflösung gegen Fedlex). Anders
/// als die Navigations-Tools brauchen sie keinen AKN-Fetcher — Discovery
/// liefert nur Kandidaten-ELIs, kein Dokument.
pub fn register_discovery_tools<C>(registry: &mut Registry, client: Arc<C>)
where
    C: SparqlClient + Send + Sync + 'static,
{
    registry.register(Arc::new(SearchLaw {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(ResolveSrNumber {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(FindRelatedTopic {
        client: Arc::clone(&client),
    }));
    // Tranche D (Staatsvertraege, Genese, Vokabular), projiziert 2026-07-02 (ADR-010).
    registry.register(Arc::new(FindTreaties {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(GetTreatyInfo {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(GetConsultations {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(GetConsultationDocuments {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(ResolveVocabularyLabel {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(ListVocabulary {
        client: Arc::clone(&client),
    }));
    registry.register(Arc::new(ExploreNode { client }));
}

// ---------------------------------------------------------------------------
// Argument-Parsing & Fehler-Mapping
// ---------------------------------------------------------------------------

/// Optionales Argument `lang` lesen (Default Deutsch).
fn arg_lang(args: &Value) -> Result<Language, ToolError> {
    match args.get("lang").and_then(Value::as_str) {
        None | Some("de") => Ok(Language::De),
        Some("fr") => Ok(Language::Fr),
        Some("it") => Ok(Language::It),
        Some("en") => Ok(Language::En),
        Some("rm") | Some("roh") => Ok(Language::Roh),
        Some(other) => Err(ToolError::InvalidArguments(format!(
            "`lang` muss de|fr|it|en|rm sein, nicht `{other}`"
        ))),
    }
}

/// Pflicht-Argument mit gegebenem Namen als String lesen.
fn arg_str<'a>(args: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("`{name}` (string) fehlt")))
}

/// Optionales `limit` lesen (Default 20, hart auf 50 gedeckelt — Discovery geht
/// live gegen Fedlex, eine grosse Liste hilft dem Agenten ohnehin nicht).
fn arg_limit(args: &Value) -> u32 {
    args.get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .clamp(1, 50) as u32
}

/// Listen-Konvention (68 §B-2): jede limit-gekappte Liste trägt ein
/// Kappungs-Signal. Da die SPARQL-Queries mit `LIMIT` arbeiten, ist die
/// Gesamtzahl unbekannt — `truncated: true` heisst „Limit erreicht, es KANN
/// mehr geben". `limit_applied` macht zusätzlich die stille Klemmung von
/// `arg_limit` sichtbar (live beobachtet: `limit: 500` → wortlos 50
/// Ergebnisse, der Agent hielt das Fenster für die Gesamtheit).
fn capped_list(key: &str, items: Vec<Value>, limit: u32) -> Value {
    let truncated = items.len() as u32 >= limit;
    json!({
        key: items,
        "truncated": truncated,
        "limit_applied": limit,
    })
}

/// JOLux-Fehler in lenkende Tool-Fehler übersetzen.
fn map_jolux(err: JoluxError) -> ToolError {
    match err {
        JoluxError::NotFound(what) => ToolError::NotFound(what),
        other => ToolError::Upstream(other.to_string()),
    }
}

/// Serialisierung eines bereits validierten Domänen-Typs.
fn to_value<T: serde::Serialize>(data: T) -> Result<Value, ToolError> {
    serde_json::to_value(data).map_err(|e| ToolError::Upstream(format!("serialize: {e}")))
}

/// Baut die Hinweis-Provenance der Anfrage selbst.
///
/// `gattung` ist ein stabiler Sentinel-ELI der Suchgattung (kein echter Erlass),
/// der nur dann zum Tragen kommt, wenn die Antwort sonst keinen Treffer-ELI
/// hätte. So bleibt das Provenance-Gate (ADR-004) auch bei null Treffern erfüllt,
/// ohne einen Treffer vorzutäuschen — `kind: "hint"` weist es als Nicht-Beleg aus.
fn query_hint(ctx: &ToolContext, gattung: &str) -> Result<Provenance, ToolError> {
    let eli = Eli::new(gattung).map_err(|e| ToolError::Upstream(e.to_string()))?;
    Ok(ctx.stamp.into_hint_provenance(eli))
}

// 68 §C-5: Die frühere Per-Hit-Provenance (`annotate_hit`) ist entfernt —
// zwanzig identische Stempel pro Antwort waren reine Token-Redundanz. Die
// Top-Level-Hinweis-Provenance der Antwort plus das `eli`-Feld jedes Treffers
// tragen dieselbe Information; verbindlich wird ein Kandidat ohnehin erst
// durch einen Norm-Beleg (ADR-006).

// ---------------------------------------------------------------------------
// Die Tools
// ---------------------------------------------------------------------------

/// JLX-RES-02. Erlass-Suche nach Titel/Stichwort. Liefert Kandidaten-ELIs.
struct SearchLaw<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for SearchLaw<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "search_law"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Sucht Bundeserlasse nach Titel/Stichwort (Discovery, JLX-RES-02). Treffer tragen in_force (geltendes Recht steht zuerst; Achtung: aufgehobene und geltende Erlasse koennen dieselbe SR-Nummer tragen). Liefert Kandidaten-ELIs als HINWEISE (kind=hint), kein Beleg — belege die Treffer anschliessend mit get_metadata/read_article.",
            "properties": {
                "query": { "type": "string", "description": "Titel-Stichwort, z.B. Energiegesetz" },
                "limit": { "type": "integer", "default": 20, "maximum": 50 },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let query = arg_str(&args, "query")?;
        let lang = arg_lang(&args)?;
        let limit = arg_limit(&args);
        let hits = search_law(self.client.as_ref(), query, lang, limit)
            .await
            .map_err(map_jolux)?;
        let annotated: Vec<Value> = hits.into_iter().filter_map(|h| to_value(h).ok()).collect();
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(capped_list("hits", annotated, limit), prov))
    }
}

/// JLX-RES-01. Löst eine SR-Nummer zu den passenden Erlassen auf.
struct ResolveSrNumber<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for ResolveSrNumber<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "resolve_sr_number"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Loest eine SR-Nummer zu Erlassen auf (Discovery, JLX-RES-01). Liefert MEHRERE Kandidaten als HINWEISE (kind=hint): SR-Nummern werden wiederverwendet, disambiguiere ueber in_force_status. Kein Beleg.",
            "properties": {
                "sr_number": { "type": "string", "description": "SR-Nummer, z.B. 730.0" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["sr_number"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let sr = arg_str(&args, "sr_number")?;
        let lang = arg_lang(&args)?;
        let hits = resolve_sr_number(self.client.as_ref(), sr, lang)
            .await
            .map_err(map_jolux)?;
        let annotated: Vec<Value> = hits.into_iter().filter_map(|h| to_value(h).ok()).collect();
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(json!({ "hits": annotated }), prov))
    }
}

/// JLX-TAX-02. Findet thematisch verwandte Erlasse (Geschwister über Taxonomie).
struct FindRelatedTopic<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for FindRelatedTopic<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "find_related_topic"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Findet Erlasse im selben Rechtsgebiet (Discovery, JLX-TAX-02). Deterministische Cross-Law-Navigation ueber die Rechtstaxonomie. Liefert Kandidaten-ELIs als HINWEISE (kind=hint), kein Beleg.",
            "properties": {
                "eli": { "type": "string", "description": "Ausgangs-ELI, z.B. eli/cc/2017/762" },
                "limit": { "type": "integer", "default": 20, "maximum": 50 }
            },
            "required": ["eli"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let raw = arg_str(&args, "eli")?;
        let eli = Eli::new(raw).map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let limit = arg_limit(&args);
        let hits = find_related_by_topic(self.client.as_ref(), &eli, limit)
            .await
            .map_err(map_jolux)?;
        let annotated: Vec<Value> = hits.into_iter().filter_map(|h| to_value(h).ok()).collect();
        // Bezug der Anfrage ist der Ausgangs-ELI selbst (existiert garantiert).
        let prov = ctx.stamp.into_hint_provenance(eli);
        Ok(Response::new(capped_list("hits", annotated, limit), prov))
    }
}

// ---------------------------------------------------------------------------
// Tests — MockSparqlClient, kein Netzwerk.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tranche D (projiziert 2026-07-02, ADR-010) — alles Kandidaten/Kontext,
// daher durchgehend HINWEIS-Provenance (kind=hint, ADR-006): Genese- und
// Vertragskontext ist Recherchematerial, kein geltendes Recht.
// ---------------------------------------------------------------------------

/// JLX-TRT-02. Staatsvertraege finden (optional nach Land/Bilateralitaet).
struct FindTreaties<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for FindTreaties<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "find_treaties"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Findet Staatsvertraege (JLX-TRT-02), optional gefiltert nach Vertragspartner-Land und Bilateralitaet. Treffer tragen Titel (angefragte Sprache), Prozess-URI und Signaturdatum. Liefert Kandidaten als HINWEISE (kind=hint), kein Beleg.",
            "properties": {
                "country_uri": { "type": "string", "description": "Optionale Land-URI des Vertragspartners (uri aus list_vocabulary, scheme_id=country)" },
                "bilateral": { "type": "boolean", "description": "Nur bilaterale (true) bzw. multilaterale (false) Vertraege" },
                "limit": { "type": "integer", "default": 20, "maximum": 50 },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            }
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let country = args.get("country_uri").and_then(Value::as_str);
        let bilateral = args.get("bilateral").and_then(Value::as_bool);
        let limit = arg_limit(&args);
        let lang = arg_lang(&args)?;
        let hits = find_treaties(self.client.as_ref(), country, bilateral, limit, lang)
            .await
            .map_err(map_jolux)?;
        let items: Vec<Value> = hits.into_iter().filter_map(|h| to_value(h).ok()).collect();
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(capped_list("hits", items, limit), prov))
    }
}

/// JLX-TRT-01. Details eines Staatsvertrags-Prozesses.
struct GetTreatyInfo<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for GetTreatyInfo<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "get_treaty_info"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Details eines Staatsvertrags-Prozesses (JLX-TRT-01): Titel, Partner, Daten, Status. URI stammt typischerweise aus find_treaties (Feld process_uri). Liefert einen HINWEIS (kind=hint) — belege den zugehoerigen Erlass separat.",
            "properties": {
                "uri": { "type": "string", "description": "Prozess-URI des Vertrags (Feld process_uri aus find_treaties)" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de", "description": "Bevorzugte Titelsprache; fehlt sie, faellt der Titel auf andere Amtssprachen zurueck" }
            },
            "required": ["uri"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let uri = arg_str(&args, "uri")?;
        let lang = arg_lang(&args)?;
        let info = get_treaty_info(self.client.as_ref(), uri, lang)
            .await
            .map_err(map_jolux)?;
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(json!({ "treaty": to_value(info)? }), prov))
    }
}

/// JLX-GEN-02. Vernehmlassungen zu einem Gesetzes-Entwurf.
struct GetConsultations<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for GetConsultations<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "get_consultations"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Vernehmlassungen zu einem Entwurf (JLX-GEN-02). Draft-URI stammt aus get_drafts. Entstehungs-Kontext als HINWEIS (kind=hint) — kein geltendes Recht.",
            "properties": {
                "draft_uri": { "type": "string", "description": "Draft-URI — Feld uri aus get_drafts, Form eli/proj/JJJJ/NNNN" }
            },
            "required": ["draft_uri"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let uri = arg_str(&args, "draft_uri")?;
        let consultations = get_consultations(self.client.as_ref(), uri)
            .await
            .map_err(map_jolux)?;
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(
            json!({ "consultations": to_value(consultations)? }),
            prov,
        ))
    }
}

/// JLX-GEN-03. Dokumente einer Vernehmlassung.
struct GetConsultationDocuments<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for GetConsultationDocuments<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "get_consultation_documents"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Dokumente einer Vernehmlassung (JLX-GEN-03): Berichte, Stellungnahmen. Entstehungs-Kontext als HINWEIS (kind=hint) — kein geltendes Recht.",
            "properties": {
                "consultation_uri": { "type": "string", "description": "Vernehmlassungs-URI — Feld uri eines Treffers aus get_consultations" }
            },
            "required": ["consultation_uri"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let uri = arg_str(&args, "consultation_uri")?;
        let documents = get_consultation_documents(self.client.as_ref(), uri)
            .await
            .map_err(map_jolux)?;
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(
            json!({ "documents": to_value(documents)? }),
            prov,
        ))
    }
}

/// JLX-VOC-01. Label eines Vokabular-Terms aufloesen.
struct ResolveVocabularyLabel<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for ResolveVocabularyLabel<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "resolve_vocabulary_label"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Loest die URI eines kontrollierten Vokabular-Terms zum sprachigen Label auf (JLX-VOC-01). Nachschlagewerk als HINWEIS (kind=hint).",
            "properties": {
                "vocab_uri": { "type": "string", "description": "Vokabular-URI aus einer frueheren Antwort (z.B. impact_type, status_uri, genre, type_document oder aus get_taxonomy)" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" }
            },
            "required": ["vocab_uri"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let uri = arg_str(&args, "vocab_uri")?;
        let lang = arg_lang(&args)?;
        let label = resolve_vocabulary_label(self.client.as_ref(), uri, lang)
            .await
            .map_err(map_jolux)?;
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(json!({ "label": label }), prov))
    }
}

/// JLX-VOC-02. Konzepte eines kontrollierten Vokabulars listen.
struct ListVocabulary<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for ListVocabulary<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "list_vocabulary"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Listet Konzepte eines kontrollierten Vokabulars (SKOS-Schema, JLX-VOC-02). Mit query gezielt nach Label suchen (z.B. scheme_id=country, query=Deutschland → Land-URI fuer find_treaties). Nachschlagewerk als HINWEIS (kind=hint).",
            "properties": {
                "scheme_id": { "type": "string", "description": "Schema-Kennung, u.a.: country, legal-taxonomy, enforcement-status, impact-type, resource-type, legal-resource-genre, treaty-type, treaty-status, consultation-status, subdivision-type, draft-document-type, legal-subject-theme-de" },
                "query": { "type": "string", "description": "Optionaler Label-Filter, case-insensitiv ueber alle Sprachen (serverseitig)" },
                "lang": { "type": "string", "enum": ["de", "fr", "it", "en", "rm"], "default": "de" },
                "limit": { "type": "integer", "default": 20, "maximum": 50 }
            },
            "required": ["scheme_id"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let scheme = arg_str(&args, "scheme_id")?;
        let lang = arg_lang(&args)?;
        let limit = arg_limit(&args);
        let query = args.get("query").and_then(Value::as_str);
        let concepts = list_vocabulary(self.client.as_ref(), scheme, lang, limit, query)
            .await
            .map_err(map_jolux)?;
        let items: Vec<Value> = concepts
            .into_iter()
            .filter_map(|c| to_value(c).ok())
            .collect();
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(capped_list("concepts", items, limit), prov))
    }
}

/// JLX-VOC-03. Nachbarschaft eines LOD-Knotens erkunden.
struct ExploreNode<C> {
    client: Arc<C>,
}

#[async_trait]
impl<C> McpTool for ExploreNode<C>
where
    C: SparqlClient + Send + Sync,
{
    fn name(&self) -> &str {
        "explore_node"
    }
    fn pool(&self) -> ToolPool {
        ToolPool::Discovery
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "description": "Erkundet die Nachbarschaft eines Knotens im Fedlex-Graphen (JLX-VOC-03): ein-/ausgehende Kanten. Explorations-Werkzeug als HINWEIS (kind=hint).",
            "properties": {
                "uri": { "type": "string", "description": "Beliebige Fedlex-URI aus einer frueheren Antwort (ELI-, Impact-, Vokabular- oder Prozess-URI)" },
                "limit": { "type": "integer", "default": 20, "maximum": 50 }
            },
            "required": ["uri"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Response<Value>, ToolError> {
        let uri = arg_str(&args, "uri")?;
        let limit = arg_limit(&args);
        let neighborhood = explore_node(self.client.as_ref(), uri, limit)
            .await
            .map_err(map_jolux)?;
        // 68 §B-2: beide Richtungen sind einzeln limit-gekappt — das Signal
        // zeigt an, ob mindestens eine Richtung am Limit hängt.
        let truncated = neighborhood.outgoing.len() as u32 >= limit
            || neighborhood.incoming.len() as u32 >= limit;
        let prov = query_hint(ctx, "eli/cc")?;
        Ok(Response::new(
            json!({
                "outgoing": to_value(neighborhood.outgoing)?,
                "incoming": to_value(neighborhood.incoming)?,
                "truncated": truncated,
                "limit_applied": limit,
            }),
            prov,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthResolver, ClaimRecord, Role, StaticAuthResolver};
    use crate::temporal::TemporalResolver;
    use fedlex_core::TransactionTime;
    use fedlex_jolux::MockSparqlClient;
    use time::macros::{date, datetime};

    /// Canned SR-Auflösung (JLX-RES-01): zwei Treffer derselben SR-Nummer.
    const SR_JSON: &str = r#"{
      "head": { "vars": ["ca", "title", "status"] },
      "results": { "bindings": [
        { "ca": { "type": "uri", "value": "https://fedlex.data.admin.ch/eli/cc/2017/762" },
          "title": { "type": "literal", "value": "Energiegesetz" },
          "status": { "type": "uri", "value": "https://fedlex.data.admin.ch/vocabulary/enforcement-status/0" } },
        { "ca": { "type": "uri", "value": "https://fedlex.data.admin.ch/eli/cc/1999/27" } }
      ] }
    }"#;

    /// Canned Titelsuche (JLX-RES-02): ein Treffer.
    const SEARCH_JSON: &str = r#"{
      "head": { "vars": ["ca", "sr", "title"] },
      "results": { "bindings": [
        { "ca": { "type": "uri", "value": "https://fedlex.data.admin.ch/eli/cc/2017/762" },
          "sr": { "type": "literal", "value": "730.0" },
          "title": { "type": "literal", "value": "Energiegesetz" } }
      ] }
    }"#;

    fn registry_with(json: &str) -> Registry {
        let client = Arc::new(MockSparqlClient::from_json(json));
        let mut r = Registry::new();
        register_discovery_tools(&mut r, client);
        r
    }

    fn ctx(role: Role) -> ToolContext {
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
    async fn discovery_tools_hidden_from_reader_visible_to_navigator() {
        let r = registry_with(SEARCH_JSON);

        let reader: Vec<String> = r
            .list_tools(Role::Reader)
            .into_iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(
            reader.is_empty(),
            "Reader darf KEIN Discovery sehen, sah: {reader:?}"
        );

        let nav: Vec<String> = r
            .list_tools(Role::Navigator)
            .into_iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        for expected in ["search_law", "resolve_sr_number", "find_related_topic"] {
            assert!(
                nav.contains(&expected.to_string()),
                "Navigator fehlt: {expected}"
            );
        }
    }

    #[tokio::test]
    async fn reader_call_on_search_law_is_gracefully_denied() {
        let r = registry_with(SEARCH_JSON);
        let out = r
            .dispatch(
                &ctx(Role::Reader),
                "search_law",
                json!({ "query": "Energie" }),
            )
            .await;
        assert!(
            out["error"].as_str().unwrap().contains("not permitted"),
            "war: {out}"
        );
    }

    #[tokio::test]
    async fn search_law_returns_hits_with_hint_provenance() {
        let r = registry_with(SEARCH_JSON);
        let out = r
            .dispatch(
                &ctx(Role::Navigator),
                "search_law",
                json!({ "query": "Energiegesetz" }),
            )
            .await;
        let hits = out["data"]["hits"].as_array().expect("hits-Array");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["eli"], "eli/cc/2017/762");
        // 68 §C-5: Per-Hit-Provenance entfernt (Token-Redundanz) — die
        // Antwort-Hülle weist sich als Hinweis aus, der Treffer trägt eli.
        assert!(hits[0].get("provenance").is_none());
        assert_eq!(out["provenance"]["kind"], "hint");
    }

    #[tokio::test]
    async fn resolve_sr_number_yields_multiple_hint_candidates() {
        let r = registry_with(SR_JSON);
        let out = r
            .dispatch(
                &ctx(Role::Navigator),
                "resolve_sr_number",
                json!({ "sr_number": "730.0" }),
            )
            .await;
        let hits = out["data"]["hits"].as_array().expect("hits-Array");
        assert_eq!(hits.len(), 2, "SR-Nummern werden wiederverwendet: {out}");
        let elis: Vec<&str> = hits.iter().map(|h| h["eli"].as_str().unwrap()).collect();
        assert!(elis.contains(&"eli/cc/2017/762"));
        assert!(elis.contains(&"eli/cc/1999/27"));
        // 68 §C-5: Disambiguierung direkt als Flag — enforcement-status/0 → true.
        let eng2016 = hits.iter().find(|h| h["eli"] == "eli/cc/2017/762").unwrap();
        assert_eq!(eng2016["in_force"], true, "{out}");
    }

    #[tokio::test]
    async fn search_law_empty_result_still_carries_hint_provenance() {
        let r = registry_with(
            r#"{ "head": { "vars": ["ca","sr","title"] }, "results": { "bindings": [] } }"#,
        );
        let out = r
            .dispatch(
                &ctx(Role::Navigator),
                "search_law",
                json!({ "query": "GibtsNicht" }),
            )
            .await;
        // Null Treffer, aber das Provenance-Gate ist erfüllt — als Hinweis.
        assert!(out["data"]["hits"].as_array().unwrap().is_empty());
        assert_eq!(out["provenance"]["kind"], "hint");
        assert!(out.get("error").is_none());
    }

    #[tokio::test]
    async fn missing_query_is_invalid_arguments() {
        let r = registry_with(SEARCH_JSON);
        let out = r
            .dispatch(&ctx(Role::Navigator), "search_law", json!({}))
            .await;
        assert!(out["error"].as_str().unwrap().contains("invalid arguments"));
    }

    // --- ADR-010-Abnahme: Dispatch-Tests der Tranche-D-Tools -------------
    // Leeres Fedlex-Resultat als Mock: Listen-Tools liefern leere Treffer
    // MIT Hinweis-Provenance (ADR-006), Einzelobjekt-Tools einen lenkenden
    // NotFound — nie einen Crash.

    const EMPTY_JSON: &str = r#"{ "head": { "vars": [] }, "results": { "bindings": [] } }"#;

    #[tokio::test]
    async fn find_treaties_empty_carries_hint_provenance() {
        let result = registry_with(EMPTY_JSON)
            .dispatch(&ctx(Role::Navigator), "find_treaties", json!({}))
            .await;
        assert_eq!(result["provenance"]["kind"], "hint", "{result}");
        assert!(result["data"]["hits"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_treaty_info_unknown_uri_is_graceful_not_found() {
        let result = registry_with(EMPTY_JSON)
            .dispatch(
                &ctx(Role::Navigator),
                "get_treaty_info",
                json!({ "uri": "https://fedlex.data.admin.ch/x" }),
            )
            .await;
        assert!(result["error"].is_string(), "{result}");
        assert!(result["hint"].is_string(), "{result}");
    }

    #[tokio::test]
    async fn get_consultations_empty_is_list_with_hint() {
        let result = registry_with(EMPTY_JSON)
            .dispatch(
                &ctx(Role::Navigator),
                "get_consultations",
                json!({ "draft_uri": "https://fedlex.data.admin.ch/d" }),
            )
            .await;
        assert_eq!(result["provenance"]["kind"], "hint", "{result}");
        assert!(
            result["data"]["consultations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn get_consultation_documents_empty_is_list_with_hint() {
        let result = registry_with(EMPTY_JSON)
            .dispatch(
                &ctx(Role::Navigator),
                "get_consultation_documents",
                json!({ "consultation_uri": "https://fedlex.data.admin.ch/c" }),
            )
            .await;
        assert_eq!(result["provenance"]["kind"], "hint", "{result}");
        assert!(result["data"]["documents"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_vocabulary_label_unknown_is_graceful_not_found() {
        let result = registry_with(EMPTY_JSON)
            .dispatch(
                &ctx(Role::Navigator),
                "resolve_vocabulary_label",
                json!({ "vocab_uri": "https://fedlex.data.admin.ch/vocabulary/x" }),
            )
            .await;
        assert!(result["error"].is_string(), "{result}");
        assert!(result["hint"].is_string(), "{result}");
    }

    #[tokio::test]
    async fn list_vocabulary_empty_is_list_with_hint() {
        let result = registry_with(EMPTY_JSON)
            .dispatch(
                &ctx(Role::Navigator),
                "list_vocabulary",
                json!({ "scheme_id": "legal-taxonomy" }),
            )
            .await;
        assert_eq!(result["provenance"]["kind"], "hint", "{result}");
        assert!(result["data"]["concepts"].as_array().unwrap().is_empty());
        // 68 §B-2: leere Liste unter dem Limit → nicht gekappt.
        assert_eq!(result["data"]["truncated"], false);
    }

    /// 68 §B-2: Erreicht eine Liste ihr Limit, wird die Kappung sichtbar —
    /// truncated=true („es KANN mehr geben") plus das effektiv angewandte
    /// Limit. Vorher: `limit: 500` → wortlos 50 Ergebnisse; der Agent hielt
    /// das Fenster für die Gesamtheit (live an der Länderliste beobachtet).
    #[tokio::test]
    async fn capped_list_signals_truncation_at_limit() {
        let one_concept = r#"{
          "head": { "vars": ["concept", "label"] },
          "results": { "bindings": [
            { "concept": { "type": "uri", "value": "https://fedlex.data.admin.ch/vocabulary/country/136" },
              "label": { "type": "literal", "value": "Deutschland" } }
          ] }
        }"#;
        let result = registry_with(one_concept)
            .dispatch(
                &ctx(Role::Navigator),
                "list_vocabulary",
                json!({ "scheme_id": "country", "limit": 1 }),
            )
            .await;
        assert_eq!(result["data"]["concepts"].as_array().unwrap().len(), 1);
        assert_eq!(result["data"]["truncated"], true, "{result}");
        assert_eq!(result["data"]["limit_applied"], 1);
    }

    #[tokio::test]
    async fn explore_node_empty_carries_hint_provenance() {
        let result = registry_with(EMPTY_JSON)
            .dispatch(
                &ctx(Role::Navigator),
                "explore_node",
                json!({ "uri": "https://fedlex.data.admin.ch/n" }),
            )
            .await;
        assert_eq!(result["provenance"]["kind"], "hint", "{result}");
        assert!(result["data"].is_object(), "{result}");
    }
}
