# ADR-010: Tranche D & G-2-Rest projizieren — 40 Tools, Matrix bleibt lückenlos

- **Status:** Accepted — umgesetzt 2026-07-02
- **Datum:** 2026-07-02
- **Kontext-Artefakt:** `crates/mcp-reader/src/{metadata,discovery,tools}.rs`, `tests/lexicon_projection.rs`
- **Betrifft:** Tool-Oberfläche des Readers (25 → **40** Tools); RBAC unverändert (vier aktive Pools)

## Kontext

Die Projektions-Matrix hielt 23 der 47 Lexikon-Primitive als „reserviert" —
Tranche D (Publikation/Genese/Staatsverträge/Vokabular) explizit „nur bei
belegtem Bedarf" ([50 §Nicht-Ziele](../50_ROADMAP_TO_PERFECT.md)). Der Bedarf
ist jetzt vom Betreiber belegt: Das System soll den **vollständigen
Funktionsraum** an der Agenten-Oberfläche anbieten.

## Entscheidung

**15 Primitive werden projiziert**, nach der Provenance-Logik von ADR-006/007:

- **JoluxMetadata (+6, `kind: norm`** — Eigenschaften eines bekannten Erlasses
  am ELI-Anker): `get_law_metadata` (RES-03), `list_expressions` (RES-05),
  `get_oc_act` (PUB-01), `get_memorial` (PUB-02), `get_fga_documents` (PUB-03),
  `get_drafts` (GEN-01). Pool 10 → 16.
- **Discovery (+7, `kind: hint`** — Kandidaten/Kontext, kein geltendes Recht):
  `find_treaties` (TRT-02), `get_treaty_info` (TRT-01), `get_consultations`
  (GEN-02), `get_consultation_documents` (GEN-03), `resolve_vocabulary_label`
  (VOC-01), `list_vocabulary` (VOC-02), `explore_node` (VOC-03). Pool 3 → 10.
  Begründung hint: Genese-/Vertragskontext und Vokabular-Lookups sind
  Recherchematerial — ein Hint darf nie als Zitat verbucht werden (ADR-006);
  URI-basierte Aufrufe tragen den Gattungs-Sentinel (`eli/cc`).
- **LocalNavigation (+2):** `extract_change_notes` (MOD-02, norm aus dem
  Dokument) und `parse_unlinked_ref` (REF-02, **hint** — ein geparster Verweis
  ist ein Suchkandidat). Pool 11 → 13.

**8 bleiben begründet ausgeschlossen:** die 6 internen Bausteine
(`resolve_manifestation`, `fetch_akn_document`, `classify_pattern`,
`resolve_eid`, `get_section_path`, `get_component_document` — sie arbeiten
unter der Haube der projizierten Tools; eine Doppel-Oberfläche wäre
Redundanz ohne Agenten-Nutzen) und die 2 RAG-Bausteine (`hollow_document`,
`chunk_document` — Ingest-Schicht von mcp-fedlex-semantic, kein Agenten-Werkzeug).

## Konsequenzen

- 47 = **39 projiziert + 8 ausgeschlossen**; +1 Composite ⇒ **40 Tools**
  (Zähl-Invarianten in `lexicon_projection.rs` nachgezogen).
- Rollen-Sichtbarkeit: `reader` 13, `navigator` +Discovery(10)+Metadata(16) = 39,
  `validator` 40. Quota-Gewichte unverändert pool-gebunden (ADR-007).
- README(s), CLAUDE.md und Website (`werkzeuge.md` × 5) ziehen die Kataloge nach.
