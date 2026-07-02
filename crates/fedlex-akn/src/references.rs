//! Primitive: Verweise (Lexikon AKN-REF-01/02, Rulebook X8/X11).

use crate::doc::provenance;
use crate::dom::AknDocument;
use crate::error::AknError;
use fedlex_core::{Response, ValidAsOf};
use serde::{Deserialize, Serialize};

/// Ein `<ref>`-Verweis im Dokument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// eId des nächsten eId-tragenden Vorfahren (Verweis-Quelle).
    pub source_eid: Option<String>,
    /// `@href` — 70.8 % absolute Fedlex-URLs, 15 % fehlen ganz (X11.2).
    /// Alle Fedlex-hrefs zeigen auf Work-Ebene (X11.3) — Sprach- und
    /// Datumsauflösung läuft über JOLux (JLX-TMP-02).
    pub href: Option<String>,
    /// Sichtbarer Linktext.
    pub label: String,
}

/// AKN-REF-01: Sammelt alle `<ref>`-Elemente des Dokuments ein
/// (Body, Präambel und Fussnoten — letztere tragen die AS-Verweise
/// der Änderungshistorie).
pub fn get_all_references(
    doc: &AknDocument,
    as_of: ValidAsOf,
) -> Result<Response<Vec<Reference>>, AknError> {
    let prov = provenance(doc, as_of)?;
    let refs = doc
        .find_all(doc.root(), "ref")
        .into_iter()
        .map(|r| Reference {
            source_eid: doc
                .parent(r)
                .and_then(|p| doc.nearest_eid(p))
                .map(str::to_string),
            href: doc.attr(r, "href").map(str::to_string),
            label: doc.text_with_notes(r),
        })
        .collect();
    Ok(Response::new(refs, prov))
}

/// Klassifikation eines href-losen Verweis-Labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefKind {
    /// Interner Artikel-Verweis (`Art. 9a`, `Artikel 7`).
    Article,
    /// SR-Nummer (`101`, `730.0`, `0.814.01`).
    SrNumber,
    /// AS-Fundstelle (`AS 2020 752`).
    AsCitation,
    /// Nicht klassifizierbar — bleibt Text.
    Unknown,
}

/// Ergebnis von [`parse_unlinked_ref`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedRef {
    /// Erkannte Verweis-Art.
    pub kind: RefKind,
    /// Extrahierter Wert (Artikelnummer, SR-Nummer, AS-Fundstelle).
    pub value: String,
    /// Artikelnummer bei [`RefKind::Article`] (`"58"`, `"9a"`), 68 §C-6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    /// Absatznummer, sofern erkennbar (`"1"`, `"2bis"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<String>,
    /// Erlass-Kürzel am Verweis-Ende (`"ParlG"`, `"EnG"`) — Brücke zu
    /// `search_law`, sofern vorhanden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub act_abbreviation: Option<String>,
    /// eId-Kandidat im Korpus-Format (`art_58/para_1`) — Brücke zu
    /// `read_element`. Kandidat, kein Beleg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eid_candidate: Option<String>,
}

impl ParsedRef {
    /// Verweis ohne strukturierte Zerlegung (SR/AS/Unknown).
    fn plain(kind: RefKind, value: String) -> Self {
        ParsedRef {
            kind,
            value,
            article: None,
            paragraph: None,
            act_abbreviation: None,
            eid_candidate: None,
        }
    }
}

/// Artikelnummern-Form: beginnt mit Ziffer, dann alphanumerisch (`58`, `9a`).
fn is_article_number(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_digit())
        && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Zerlegt den Artikel-Verweis strukturiert (68 §C-6): «58 Abs. 1 ParlG» →
/// Artikel 58, Absatz 1, Kürzel «ParlG», eId-Kandidat `art_58/para_1`.
/// Konservativ — was nicht sicher erkennbar ist, bleibt `None`; `value`
/// trägt weiterhin den Rohtext.
fn parse_article_ref(value: String) -> ParsedRef {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let strip = |s: &str| s.trim_end_matches([',', ';', '.']).to_string();

    let article = tokens
        .first()
        .map(|t| strip(t))
        .filter(|t| is_article_number(t));

    let mut paragraph = None;
    for (i, tok) in tokens.iter().enumerate() {
        if matches!(*tok, "Abs." | "Absatz")
            && let Some(next) = tokens.get(i + 1)
        {
            let p = strip(next);
            if p.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                paragraph = Some(p);
                break;
            }
        }
    }

    // Kürzel: letztes Token, beginnt mit Grossbuchstabe, rein alphanumerisch
    // (ParlG, EnG, BV) — nie das Artikel-Token selbst.
    let act_abbreviation = (tokens.len() >= 2)
        .then(|| tokens.last().map(|t| strip(t)))
        .flatten()
        .filter(|t| {
            t.chars().next().is_some_and(char::is_uppercase) && t.chars().all(char::is_alphanumeric)
        });

    let eid_candidate = article.as_ref().map(|a| {
        let base = format!("art_{}", a.to_lowercase());
        match &paragraph {
            Some(p) => format!("{base}/para_{}", p.to_lowercase()),
            None => base,
        }
    });

    ParsedRef {
        kind: RefKind::Article,
        value,
        article,
        paragraph,
        act_abbreviation,
        eid_candidate,
    }
}

/// AKN-REF-02: Klassifiziert die 15 % href-losen Verweis-Labels (X11.2)
/// per Heuristik und zerlegt Artikel-Verweise strukturiert (68 §C-6).
/// Reine Textanalyse, bewusst konservativ — was nicht sicher erkennbar
/// ist, bleibt `Unknown` bzw. `None` statt falsch verlinkt.
pub fn parse_unlinked_ref(label: &str) -> ParsedRef {
    let t = label.trim();
    // Artikel-Verweise: "Art. 9a", "Artikel 7 Absatz 2", "Art. 58 Abs. 1 ParlG".
    for prefix in ["Art.", "Artikel"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let value = rest.trim().to_string();
            if !value.is_empty() {
                return parse_article_ref(value);
            }
        }
    }
    // AS-Fundstellen: "AS 2020 752".
    if let Some(rest) = t.strip_prefix("AS ") {
        let rest = rest.trim();
        if rest.len() >= 4 && rest[..4].chars().all(|c| c.is_ascii_digit()) {
            return ParsedRef::plain(RefKind::AsCitation, rest.to_string());
        }
    }
    // SR-Nummern: nur Ziffern und Punkte ("101", "0.814.01").
    if !t.is_empty()
        && t.chars().all(|c| c.is_ascii_digit() || c == '.')
        && t.chars().any(|c| c.is_ascii_digit())
    {
        return ParsedRef::plain(RefKind::SrNumber, t.to_string());
    }
    ParsedRef::plain(RefKind::Unknown, t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdoc::{sample, stichtag};

    #[test]
    fn collects_refs_with_and_without_href() {
        let doc = sample();
        let resp = get_all_references(&doc, stichtag()).unwrap();
        let refs = resp.data();
        // Präambel (2: BV-ref + Note-ref) + Artikel-Note (1) + 2 href-lose in items.
        assert_eq!(refs.len(), 5);
        let with_href = refs.iter().filter(|r| r.href.is_some()).count();
        let without = refs.iter().filter(|r| r.href.is_none()).count();
        assert_eq!(with_href, 3);
        assert_eq!(without, 2);
        let item_ref = refs
            .iter()
            .find(|r| r.source_eid.as_deref() == Some("art_2/para_1/list_1/item_a"))
            .unwrap();
        assert_eq!(item_ref.label, "Art. 1");
        assert!(item_ref.href.is_none());
    }

    #[test]
    fn parses_unlinked_labels() {
        assert_eq!(
            parse_unlinked_ref("Art. 9a"),
            ParsedRef {
                kind: RefKind::Article,
                value: "9a".into(),
                article: Some("9a".into()),
                paragraph: None,
                act_abbreviation: None,
                eid_candidate: Some("art_9a".into()),
            }
        );
        assert_eq!(
            parse_unlinked_ref("Artikel 7 Absatz 2").kind,
            RefKind::Article
        );
        assert_eq!(parse_unlinked_ref("101").kind, RefKind::SrNumber);
        assert_eq!(parse_unlinked_ref("101").value, "101");
        assert_eq!(parse_unlinked_ref("0.814.01").kind, RefKind::SrNumber);
        assert_eq!(parse_unlinked_ref("AS 2020 752").kind, RefKind::AsCitation);
        assert_eq!(parse_unlinked_ref("AS 2020 752").value, "2020 752");
        assert_eq!(parse_unlinked_ref("siehe oben").kind, RefKind::Unknown);
    }

    /// 68 §C-6: «Art. 58 Abs. 1 ParlG» wird strukturiert zerlegt — vorher
    /// bekam der Agent nur {Article, "58 Abs. 1 ParlG"} und musste selbst
    /// weiterparsen. eid_candidate ist die Brücke zu read_element,
    /// act_abbreviation die zu search_law.
    #[test]
    fn article_refs_decompose_into_reusable_parts() {
        let p = parse_unlinked_ref("Art. 58 Abs. 1 ParlG");
        assert_eq!(p.kind, RefKind::Article);
        assert_eq!(p.article.as_deref(), Some("58"));
        assert_eq!(p.paragraph.as_deref(), Some("1"));
        assert_eq!(p.act_abbreviation.as_deref(), Some("ParlG"));
        assert_eq!(p.eid_candidate.as_deref(), Some("art_58/para_1"));

        // "Artikel"-Langform mit Absatz, ohne Kürzel.
        let p = parse_unlinked_ref("Artikel 7 Absatz 2");
        assert_eq!(p.article.as_deref(), Some("7"));
        assert_eq!(p.paragraph.as_deref(), Some("2"));
        assert_eq!(p.act_abbreviation, None, "Ziffer ist kein Kürzel");
        assert_eq!(p.eid_candidate.as_deref(), Some("art_7/para_2"));

        // Konservativ: Aufzählung («Art. 5, 7, 12») liefert nur den ersten
        // Artikel strukturiert, der Rohtext bleibt vollständig in value.
        let p = parse_unlinked_ref("Art. 5, 7, 12");
        assert_eq!(p.article.as_deref(), Some("5"));
        assert_eq!(p.value, "5, 7, 12");
        assert_eq!(p.paragraph, None);
    }
}
