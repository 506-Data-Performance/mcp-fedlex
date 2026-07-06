//! Primitive: Erlass-Suche nach Titel/Stichwort (Rulebook J3).

use crate::client::{Language, PREFIXES, SparqlClient, val};
use crate::{FEDLEX_BASE, error::JoluxError};
use fedlex_core::{ValidAsOf, swiss_today};
use serde::{Deserialize, Serialize};

/// Ein Such-Treffer: ein Erlass, der zum Suchbegriff passt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LawHit {
    /// ELI des Erlasses (relativ, `eli/cc/...`).
    pub eli: String,
    /// SR-Nummer, sofern als `historicalLegalId` am Erlass vorhanden. Neue
    /// Konsolidierungen (z. B. das nDSG, `eli/cc/2022/491`) tragen kein
    /// SR-Literal mehr — dann `None`; auflösbar über `get_law_metadata`
    /// oder rückwärts über `resolve_sr_number` (Taxonomie-Pfad). Der
    /// Taxonomie-Join wäre hier zu teuer (live +0.65 s je Suche, 68 §F-34).
    pub sr_number: Option<String>,
    /// Titel des Treffers.
    pub title: String,
    /// Geltung **zum Stichtag `as_of`** (68 §F-3): primär aus den Datums-
    /// feldern (`entry <= as_of < min(noLonger, endApplicability)`, J3.2);
    /// fehlen sie, sagt der heutige `jolux:inForceStatus` nur für den
    /// heutigen Stichtag etwas aus — für andere Stichtage bleibt das Feld
    /// ehrlich leer (`None`) statt heutigen Status als historischen
    /// auszugeben. Dann [`check_in_force`] fragen.
    ///
    /// [`check_in_force`]: crate::temporal::check_in_force
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_force: Option<bool>,
}

// `historicalLegalId` ist OPTIONAL (68 §F-1): neue Konsolidierungen (nDSG,
// `eli/cc/2022/491`) tragen kein SR-Literal am Erlass — als Pflicht-Pattern
// schloss es genau das geltende Recht von der Titelsuche aus (der Explorer-
// Blocker: «Datenschutzgesetz» fand ausschliesslich aufgehobene Erlasse).
const SEARCH_Q: &str = r#"SELECT DISTINCT ?ca ?sr ?title ?status ?entry ?noLonger ?endApp WHERE {
  ?ca a jolux:ConsolidationAbstract ;
      jolux:isRealizedBy ?expr .
  ?expr jolux:language <__LANGURI__> ;
        jolux:title ?title .
  OPTIONAL { ?ca jolux:historicalLegalId ?sr }
  OPTIONAL { ?ca jolux:inForceStatus ?status }
  OPTIONAL { ?ca jolux:dateEntryInForce ?entry }
  OPTIONAL { ?ca jolux:dateNoLongerInForce ?noLonger }
  OPTIONAL { ?ca jolux:dateEndApplicability ?endApp }
  FILTER(CONTAINS(LCASE(STR(?title)), LCASE("__QUERY__")))
} LIMIT __LIMIT__"#;

/// Geltung eines Treffers **zum Stichtag** (68 §F-3, Doppel-Logik J3.2/J3.3).
///
/// Primär entscheiden die Datumsfelder — sie gelten für jeden Stichtag.
/// Fehlen sie, trägt der Graph nur den *heutigen* `inForceStatus`; der ist
/// ausschliesslich für den heutigen Stichtag eine Aussage. Für historische
/// Stichtage hiesse «Status heute in Kraft» eben nicht «galt damals» —
/// exakt so wählte die dokumentierte Disambiguierung im Explorer-Lauf das
/// am Stichtag noch nicht existierende DSG 2022. Dann lieber `None`.
pub(crate) fn in_force_at(
    status_uri: Option<&str>,
    entry: Option<&str>,
    no_longer: Option<&str>,
    end_app: Option<&str>,
    as_of: ValidAsOf,
) -> Option<bool> {
    if let Some(entry) = entry {
        // ISO-Datums-Strings vergleichen lexikografisch korrekt.
        let day = as_of.to_string();
        let started = entry <= day.as_str();
        let ended = [no_longer, end_app]
            .into_iter()
            .flatten()
            .any(|d| d <= day.as_str());
        Some(started && !ended)
    } else if as_of.date() == swiss_today() {
        status_uri.map(|s| s.ends_with("/0"))
    } else {
        None
    }
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
    as_of: ValidAsOf,
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
    let mut hits: Vec<LawHit> = Vec::new();
    for b in res.bindings() {
        let Some(ca) = val(b, "ca") else { continue };
        let Some(title) = val(b, "title") else {
            continue;
        };
        let hit = LawHit {
            eli: ca.strip_prefix(FEDLEX_BASE).unwrap_or(ca).to_string(),
            sr_number: val(b, "sr").map(str::to_string),
            title: title.to_string(),
            in_force: in_force_at(
                val(b, "status"),
                val(b, "entry"),
                val(b, "noLonger"),
                val(b, "endApp"),
                as_of,
            ),
        };
        // Dedup pro ELI (68 §F-19): Mehrfach-Bindings (z. B. mehrere
        // Expressions) lieferten denselben Erlass wortgleich doppelt —
        // DISTINCT griff nicht, weil sich Nebenvariablen unterscheiden.
        // Erste Zeile gewinnt; fehlende sr_number wird nachgetragen.
        match hits.iter_mut().find(|h| h.eli == hit.eli) {
            Some(existing) => {
                if existing.sr_number.is_none() {
                    existing.sr_number = hit.sr_number;
                }
            }
            None => hits.push(hit),
        }
    }
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
    use time::macros::date;

    /// Heutiger Stichtag (Schweizer Zeit) — für Tests des Status-Fallbacks.
    fn today() -> ValidAsOf {
        ValidAsOf::new(swiss_today())
    }

    const FIXTURE: &str = r#"{
      "head": {"vars": ["ca","sr","title","status","entry","noLonger","endApp"]},
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
        let hits = search_law(&client, "energie", Language::De, 10, today())
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
        assert!(q.contains("jolux:dateEntryInForce"));
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
        let hits = search_law(&client, "test", Language::De, 10, today())
            .await
            .unwrap();
        assert_eq!(hits[0].in_force, None);
    }

    /// 68 §F-3: Die Explorer-Falle. Ein Erlass, der HEUTE gilt
    /// (Status /0, entry 2023-09-01), war am Stichtag 2020-06-01 noch nicht
    /// in Kraft — `in_force` muss den Stichtag spiegeln, nicht den heutigen
    /// Status. Der damals geltende Alt-Erlass (entry 1993, noLonger 2023)
    /// ist am Stichtag `true` und sortiert zuerst.
    #[tokio::test]
    async fn in_force_reflects_as_of_not_today() {
        let client = MockSparqlClient::from_json(
            r#"{"head":{"vars":["ca","sr","title","status","entry","noLonger","endApp"]},"results":{"bindings":[
              {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/2022/491"},
               "sr":{"type":"literal","value":"235.1"},
               "title":{"type":"literal","xml:lang":"de","value":"Bundesgesetz ueber den Datenschutz (DSG)"},
               "status":{"type":"uri","value":"https://fedlex.data.admin.ch/vocabulary/enforcement-status/0"},
               "entry":{"type":"literal","value":"2023-09-01"}},
              {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/1993/1945_1945_1945"},
               "sr":{"type":"literal","value":"235.1"},
               "title":{"type":"literal","xml:lang":"de","value":"Bundesgesetz ueber den Datenschutz (DSG)"},
               "status":{"type":"uri","value":"https://fedlex.data.admin.ch/vocabulary/enforcement-status/3"},
               "entry":{"type":"literal","value":"1993-07-01"},
               "noLonger":{"type":"literal","value":"2023-09-01"}}
            ]}}"#,
        );
        let as_of = ValidAsOf::new(date!(2020 - 06 - 01));
        let hits = search_law(&client, "Datenschutz", Language::De, 10, as_of)
            .await
            .unwrap();
        // Am Stichtag gilt der Alt-Erlass — er steht zuerst.
        assert_eq!(hits[0].eli, "eli/cc/1993/1945_1945_1945");
        assert_eq!(hits[0].in_force, Some(true));
        assert_eq!(hits[1].eli, "eli/cc/2022/491");
        assert_eq!(hits[1].in_force, Some(false));
    }

    /// 68 §F-3: Ohne Datumsfelder sagt der heutige Status nichts über einen
    /// historischen Stichtag — ehrlich `None` statt heutigen Status als
    /// damalige Geltung auszugeben.
    #[tokio::test]
    async fn status_only_hit_is_unknown_for_past_as_of() {
        let client = MockSparqlClient::from_json(FIXTURE);
        let hits = search_law(
            &client,
            "energie",
            Language::De,
            10,
            ValidAsOf::new(date!(2020 - 06 - 01)),
        )
        .await
        .unwrap();
        assert!(hits.iter().all(|h| h.in_force.is_none()));
    }

    /// 68 §F-1 (Explorer-Blocker): Neue Konsolidierungen ohne SR-Literal
    /// (nDSG) müssen per Titel auffindbar sein — `historicalLegalId` ist
    /// OPTIONAL, `sr_number` bleibt dann leer statt den Treffer zu schlucken.
    #[tokio::test]
    async fn finds_law_without_sr_literal() {
        let client = MockSparqlClient::from_json(
            r#"{"head":{"vars":["ca","sr","title","status","entry"]},"results":{"bindings":[
              {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/2022/491"},
               "title":{"type":"literal","xml:lang":"de","value":"Bundesgesetz ueber den Datenschutz (Datenschutzgesetz, DSG)"},
               "status":{"type":"uri","value":"https://fedlex.data.admin.ch/vocabulary/enforcement-status/0"},
               "entry":{"type":"literal","value":"2023-09-01"}}
            ]}}"#,
        );
        let hits = search_law(&client, "Datenschutzgesetz", Language::De, 10, today())
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].eli, "eli/cc/2022/491");
        assert_eq!(hits[0].sr_number, None);
        assert_eq!(hits[0].in_force, Some(true));
        // Das Pflicht-Pattern ist wirklich weg.
        let q = client.last_query().unwrap();
        assert!(q.contains("OPTIONAL { ?ca jolux:historicalLegalId ?sr }"));
    }

    /// 68 §F-19: Mehrfach-Bindings desselben Erlasses (z. B. über mehrere
    /// Expressions) kollabieren zu EINEM Treffer; sr_number wird gemerged.
    #[tokio::test]
    async fn duplicate_rows_collapse_to_one_hit() {
        let client = MockSparqlClient::from_json(
            r#"{"head":{"vars":["ca","sr","title","status","entry"]},"results":{"bindings":[
              {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/2019/112"},
               "title":{"type":"literal","xml:lang":"de","value":"Schengen-Datenschutzgesetz"},
               "entry":{"type":"literal","value":"2019-03-01"}},
              {"ca":{"type":"uri","value":"https://fedlex.data.admin.ch/eli/cc/2019/112"},
               "sr":{"type":"literal","value":"235.3"},
               "title":{"type":"literal","xml:lang":"de","value":"Schengen-Datenschutzgesetz"},
               "entry":{"type":"literal","value":"2019-03-01"}}
            ]}}"#,
        );
        let hits = search_law(&client, "Datenschutz", Language::De, 10, today())
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "Duplikat nicht kollabiert: {hits:?}");
        // Die sr_number aus der zweiten Zeile ist nachgetragen.
        assert_eq!(hits[0].sr_number.as_deref(), Some("235.3"));
    }

    #[tokio::test]
    async fn neutralizes_injection_in_query() {
        let client = MockSparqlClient::from_json(FIXTURE);
        let _ = search_law(&client, r#"a") } INJECT {"#, Language::De, 5, today())
            .await
            .unwrap();
        let q = client.last_query().unwrap();
        // Die Breakout-Sequenz (Quote+schliessende Klammer) darf nicht roh vorkommen ...
        assert!(!q.contains("\") }"), "Breakout-Sequenz nicht neutralisiert");
        // ... der Text bleibt aber als harmloses Literal im CONTAINS erhalten.
        assert!(q.contains("INJECT"));
    }
}
