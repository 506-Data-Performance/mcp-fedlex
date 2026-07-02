//! Primitive: Erlass-Suche nach Titel/Stichwort (Rulebook J3).

use crate::client::{Language, PREFIXES, SparqlClient, val};
use crate::{FEDLEX_BASE, error::JoluxError};
use serde::{Deserialize, Serialize};

/// Ein Such-Treffer: ein Erlass, der zum Suchbegriff passt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LawHit {
    /// ELI des Erlasses (relativ, `eli/cc/...`).
    pub eli: String,
    /// SR-Nummer, sofern vorhanden.
    pub sr_number: Option<String>,
    /// Titel des Treffers.
    pub title: String,
    /// Geltung laut `jolux:inForceStatus` (68 §C-5): `Some(true)` = in Kraft,
    /// `Some(false)` = nicht (mehr) in Kraft, `None` = kein Status im Graphen
    /// (J3.3: 15 % der CAs) — dann [`check_in_force`] fragen, das über die
    /// Datumsfelder entscheidet.
    ///
    /// [`check_in_force`]: crate::temporal::check_in_force
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_force: Option<bool>,
}

const SEARCH_Q: &str = r#"SELECT DISTINCT ?ca ?sr ?title ?status WHERE {
  ?ca a jolux:ConsolidationAbstract ;
      jolux:historicalLegalId ?sr ;
      jolux:isRealizedBy ?expr .
  ?expr jolux:language <__LANGURI__> ;
        jolux:title ?title .
  OPTIONAL { ?ca jolux:inForceStatus ?status }
  FILTER(CONTAINS(LCASE(STR(?title)), LCASE("__QUERY__")))
} LIMIT __LIMIT__"#;

/// Leitet aus der Status-URI die Geltung ab (`.../enforcement-status/0` = in
/// Kraft). Kein Status → `None`, nie geraten.
pub(crate) fn derive_in_force(status_uri: Option<&str>) -> Option<bool> {
    status_uri.map(|s| s.ends_with("/0"))
}

/// Sucht Erlasse, deren Titel den Suchbegriff enthält (case-insensitive).
///
/// **Live-verifiziert (2026-06-10):** Der amtliche Titel liegt auf der
/// Expression **direkt am CA** (`<CA> jolux:isRealizedBy ?expr`). Die
/// Expressions der Consolidations tragen nur technische Labels
/// (`"Consolidation: 730.0 - 2018-01-01"`) und sind für die Suche unbrauchbar.
///
/// **Discovery-Funktion ohne Provenance** — liefert Kandidaten, auf denen dann
/// provenance-tragende Primitive (`get_law_metadata`, `get_article_text`)
/// aufsetzen. Geltendes Recht steht zuerst (68 §C-5). Der Suchbegriff wird vor
/// der Einbettung entschärft (Anführungszeichen/Backslash entfernt), damit er
/// die Query nicht zerbricht (kein SPARQL-Injection).
pub async fn search_law(
    client: &impl SparqlClient,
    query: &str,
    lang: Language,
    limit: u32,
) -> Result<Vec<LawHit>, JoluxError> {
    let safe = query.replace(['"', '\\'], " ");
    let sparql = format!(
        "{PREFIXES}{}",
        SEARCH_Q
            .replace("__LANGURI__", lang.vocab_uri())
            .replace("__QUERY__", &safe)
            .replace("__LIMIT__", &limit.to_string())
    );
    let res = client.query(&sparql).await?;
    let mut hits: Vec<LawHit> = res
        .bindings()
        .iter()
        .filter_map(|b| {
            let ca = val(b, "ca")?;
            let title = val(b, "title")?.to_string();
            Some(LawHit {
                eli: ca.strip_prefix(FEDLEX_BASE).unwrap_or(ca).to_string(),
                sr_number: val(b, "sr").map(str::to_string),
                title,
                in_force: derive_in_force(val(b, "status")),
            })
        })
        .collect();
    // 68 §C-5: Geltendes Recht zuerst. Live stand das aufgehobene EnG 1998
    // VOR dem geltenden EnG 2016 (beide SR 730.0) — die klassische
    // Agenten-Falsch-Wahl «erster Treffer = richtig». Aufgehobenes ans Ende,
    // Unbekanntes in die Mitte (stabile Sortierung, Reihenfolge sonst erhalten).
    hits.sort_by_key(|h| match h.in_force {
        Some(true) => 0u8,
        None => 1,
        Some(false) => 2,
    });
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::MockSparqlClient;

    const FIXTURE: &str = r#"{
      "head": {"vars": ["ca","sr","title","status"]},
      "results": {"bindings": [
        {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/1999/27"},
         "sr":{"type":"literal","value":"730.0"},
         "title":{"type":"literal","xml:lang":"de","value":"Energiegesetz vom 26. Juni 1998 (EnG)"},
         "status":{"type":"uri","value":"https://fedlex.data.admin.ch/vocabulary/enforcement-status/3"}},
        {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/2017/762"},
         "sr":{"type":"literal","value":"730.0"},
         "title":{"type":"literal","xml:lang":"de","value":"Energiegesetz (EnG)"},
         "status":{"type":"uri","value":"https://fedlex.data.admin.ch/vocabulary/enforcement-status/0"}}
      ]}
    }"#;

    /// 68 §C-5: Das ist exakt die live beobachtete Falle — das aufgehobene
    /// EnG 1998 kam VOR dem geltenden EnG 2016 (beide SR 730.0), und nichts
    /// im Treffer unterschied sie. Jetzt: in_force-Flag + geltend-zuerst.
    #[tokio::test]
    async fn hits_carry_in_force_and_current_law_sorts_first() {
        let client = MockSparqlClient::from_json(FIXTURE);
        let hits = search_law(&client, "energie", Language::De, 10)
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
        // Das geltende Gesetz zuerst, obwohl es im Roh-Resultat an zweiter
        // Stelle stand.
        assert_eq!(hits[0].eli, "eli/cc/2017/762");
        assert_eq!(hits[0].in_force, Some(true));
        assert_eq!(hits[0].sr_number.as_deref(), Some("730.0"));
        assert_eq!(hits[1].eli, "eli/cc/1999/27");
        assert_eq!(hits[1].in_force, Some(false));

        let q = client.last_query().unwrap();
        assert!(q.contains("ConsolidationAbstract"));
        assert!(q.contains("jolux:inForceStatus"));
        assert!(q.contains("LIMIT 10"));
        assert!(q.contains(r#"LCASE("energie")"#));
    }

    /// Kein Status im Graphen (J3.3) → `None`, sortiert zwischen geltend
    /// und aufgehoben.
    #[tokio::test]
    async fn missing_status_is_none_not_guessed() {
        let client = MockSparqlClient::from_json(
            r#"{"head":{"vars":["ca","sr","title","status"]},"results":{"bindings":[
              {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/2000/1"},
               "sr":{"type":"literal","value":"111"},
               "title":{"type":"literal","xml:lang":"de","value":"Testgesetz"}}
            ]}}"#,
        );
        let hits = search_law(&client, "test", Language::De, 10).await.unwrap();
        assert_eq!(hits[0].in_force, None);
    }

    #[tokio::test]
    async fn neutralizes_injection_in_query() {
        let client = MockSparqlClient::from_json(FIXTURE);
        let _ = search_law(&client, r#"a") } INJECT {"#, Language::De, 5)
            .await
            .unwrap();
        let q = client.last_query().unwrap();
        // Die Breakout-Sequenz (Quote+schliessende Klammer) darf nicht roh vorkommen ...
        assert!(!q.contains("\") }"), "Breakout-Sequenz nicht neutralisiert");
        // ... der Text bleibt aber als harmloses Literal im CONTAINS erhalten.
        assert!(q.contains("INJECT"));
    }
}
