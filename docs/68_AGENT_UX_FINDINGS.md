# 68 — Agent-UX-Findings (Dogfooding-Session)

> **Was dieses Dokument ist.** Ergebnis einer Dogfooding-Session vom **2026-07-02**:
> Eine KI (Claude) hat den Reader selbst gestartet und als Agent verwendet — so, wie
> die Zielgruppe ihn erlebt. Szenario: reale Rechtsrecherche («Was sagt das
> Energiegesetz zur Einspeisevergütung, seit wann gilt Art. 19?») plus Live-Erstkontakt
> aller 15 ADR-010-Tools, Fehlerpfade und Ketten-Tests (Output von Tool N → Input von
> Tool N+1). ~35 Live-Calls gegen Fedlex, Binary nativ + Redis-Container, Dev-Token
> (Validator), `/rpc`. Jeder Punkt trägt Beleg aus der Session.
> **Status-Werte:** 🔴 offen · 🟡 in Arbeit · 🟢 erledigt · ⚪ verworfen.
>
> **Abarbeitung 2026-07-02:** Alle Punkte umgesetzt (`961eca8…0c38627`,
> jeder mit Test-Abnahme; SPARQL-Änderungen zusätzlich live gegen Fedlex
> verifiziert; CI-Gleichlauf durchgehend grün, 25 Suiten). A-2 ist als
> **ADR-011** festgehalten. Einzig C-9 (Quota-Sichtbarkeit) ist bewusst
> ⚪ verworfen. B-1 löst damit auch [67 P-5](67_HARDENING_AND_SOTA_ROADMAP.md)
> für den Live-Pfad.

---

## 0. Gesamtbild

Der Server fühlt sich als Agent **stabil, schnell und vertrauenswürdig** an: kein
Absturz, keine falsche Quota-Sperre, jede Antwort JSON-konform, Fehler kommen in-band
mit `isError` + Hinweis. Die norm/hint-Trennung wirkt tatsächlich epistemisch führend —
als Agent behandelt man einen `hint`-Treffer *automatisch* anders als einen Norm-Beleg,
weil die Struktur es erzwingt. Das ist das Alleinstellungsmerkmal und es funktioniert.

**Der Kernbefund:** Die größten Reibungen liegen nicht in den Primitiven, sondern in
der **Sichtbarkeitsschicht zwischen Server und Modell**. Ein Standard-MCP-Host zeigt
dem Modell `tool.description` (Top-Level) — dort steht bei allen 40 Tools nichts; die
Beschreibungen stecken nur in `inputSchema.description`. Und der Stichtag (`as_of`),
das Kernversprechen des Produkts, liegt auf `params`-Ebene außerhalb von `arguments` —
ein Agent hinter einem Standard-Host **kann ihn nicht setzen**, und ein falsch
platziertes `as_of` in `arguments` wird kommentarlos ignoriert.

### Verifiziert stark (nicht anfassen)

| Geprüft | Beleg (Session) |
|---|---|
| `search_text`: kompakte eid+Snippet-Treffer, direkt weiterverwendbar | 20 Treffer, 7 KB, jede Fundstelle mit `eid` |
| `compare_versions`: added/changed/removed + fertiges Markdown | Diff EnG heute↔2023-01-01, 3 KB |
| `get_metadata`-Pattern-Block (`article_count`, `pattern`) als Orientierung | EnG: 105 Artikel, «Structured» |
| Fehler in-band, feld-genau bei Argumentfehlern | `invalid arguments: 'label' (string) fehlt` |
| Leere Ergebnisse als leere Liste **mit** Provenance (kein Fehler) | `get_consultations` → `{"consultations":[]}`, kind-korrekt |
| Ketten schließen: `get_drafts.uri` → `get_consultations.draft_uri`; `list_vocabulary` → `find_treaties.country_uri` | eli/proj/2012/1295; country/136 → 3 Verträge CH–DE |
| `resolve_vocabulary_label` löst produktiv auf | impact-type/1 → «Änderung» |
| Health/Startup sauber, ~35 Live-Calls ohne Quota-Fehlalarm | `readyz` 200, durchgehend |

---

## A — Sichtbarkeitsschicht (höchster Hebel)

### A-1 · Tool-Beschreibungen fehlen auf Top-Level 🟢 (`961eca8`)

- **Beleg:** `tools/list`: `.tools[].description` ist bei allen 40 Tools `null`; der
  Text existiert, aber nur in `inputSchema.description` (und im Legacy-Duplikat `schema`).
- **Wirkung:** MCP-Hosts präsentieren dem Modell das Top-Level-Feld. Über jeden
  Standard-Host (Claude, Inspector, LangChain-Adapter) erscheinen 40 Tools **ohne
  jede Erklärung** — das Modell rät aus Namen und Parametern. Der beste Text des
  Systems (inkl. norm/hint-Hinweisen wie bei `search_law`) erreicht das Modell nie.
- **Behandlung:** In `tools/list` denselben String zusätzlich als `description`
  emittieren. Eine Zeile pro Tool-Definition, kein Breaking Change.

### A-2 · Stichtag für Agenten unerreichbar; falsches `as_of` wird verschluckt 🟢 (`0226cdc`, ADR-011)

- **Beleg:** `as_of` wird nur auf `params`-Ebene gelesen (`transport.rs:445`).
  `read_article` mit `as_of:"2024-01-01"` **in `arguments`** → Provenance
  `valid_as_of: 2026-07-02` (heute), kommentarlos. `check_in_force` («war eine Norm
  zum Stichtag in Kraft») hat **gar keinen Datums-Parameter** im Schema.
- **Wirkung:** Ein Agent hinter einem Standard-Host füllt nur `arguments` — er kann
  den Stichtag **nie** setzen. Das Kernversprechen «zeitpunktgenau» ist für generische
  MCP-Clients unerreichbar; schlimmer: der Agent *glaubt*, historisch gefragt zu haben,
  und bekommt heutiges Recht mit Norm-Provenance. Nur die Provenance-Diskrepanz kann
  ihn retten — wenn er sie prüft.
- **Behandlung:** `as_of` als optionales Argument der zeitsensitiven Tools aufnehmen
  (validiert wie `params.as_of`; Vorrang: `params.as_of` > `arguments.as_of` für
  bestehende Clients). ADR-004 bleibt intakt: Provenance stempelt weiterhin
  serverseitig das *effektiv verwendete* Datum; ein Stichtag ist Query-Parameter,
  keine Identität (ADR-002 unberührt). Präzedenz existiert: `compare_versions` nimmt
  `compare_to` heute schon als ISO-Datum in `arguments`. Entscheidung als **ADR-011**
  festhalten.

### A-3 · `initialize` ohne `instructions` 🟢 (`7e2a951`)

- **Beleg:** `initialize`-Result enthält `protocolVersion`, `serverInfo`,
  `capabilities` — kein `instructions`-Feld.
- **Wirkung:** Der Spec-Platz, an dem ein Server dem Agenten die Hausordnung mitgibt
  (Hosts injizieren ihn in den System-Prompt), bleibt leer. Genau dieses System hat
  eine Hausordnung, die das Modell kennen muss: norm vs. hint, typischer Workflow
  (`search_law` → `get_metadata`/`check_in_force` → `read_article`), eid-Format
  (`art_19`, `art_19/para_2`), Stichtag-Semantik, ELI-Formen (Werk-ELI `eli/cc/…` vs.
  AS-ELI `eli/oc/…`).
- **Behandlung:** Einen kompakten `instructions`-Absatz (≤ ~150 Wörter) im
  `initialize`-Result ausliefern; Inhalt aus README §Tools destillieren.

---

## B — Antwort-Ökonomie (Kontext ist die knappste Ressource des Agenten)

### B-1 · Große Antworten sprengen das Kontext-Budget (Live-Zahlen zu 67 P-5) 🟢 (`3eb488c`)

- **Beleg (EnG, mittelgroßes Gesetz):** `read_document` **210 KB**, `get_structure`
  **95 KB** (bis Absatz-Ebene, mit leeren `children: []`-Arrays), `get_references`
  **82 KB**. Durch das ADR-009-Envelope (text-Block ≙ structuredContent) verdoppelt
  sich jedes Payload auf der Leitung; Hosts, die beide rendern, zahlen doppelt.
- **Wirkung:** Ein einziger `read_document`-Aufruf kostet ~50k Tokens — mehr als die
  meisten Agenten-Budgets für den gesamten Recherche-Schritt. `get_structure` als
  *Orientierungs*-Tool ist zum Orientieren zu schwer.
- **Behandlung:** = [67 P-5](67_HARDENING_AND_SOTA_ROADMAP.md), jetzt mit Zahlen.
  Konkret: (a) `get_structure` bekommt `depth` (Default: Artikel-Ebene) und lässt
  leere `children` weg; (b) `read_document`/`get_references` paginieren
  (`xml_engine::paginate` existiert bereits getestet); (c) optional `max_bytes`-Budget
  pro Call.

### B-2 · Stille Kappung ohne Truncation-Signal 🟢 (`84acc1b`)

- **Beleg:** `list_vocabulary` mit `limit: 500` → exakt 50 Konzepte, kein Marker.
  `search_text` → exakt 20 Treffer (Default) als nacktes Array, kein `total`.
- **Wirkung:** Der Agent hält das Fenster für die Gesamtheit. Konkret in der Session:
  Wäre «Deutschland» nicht zufällig in den ersten 50 Länder-Konzepten gewesen, wäre
  das Fazit «keine URI vorhanden» gewesen — eine stille Falschauskunft.
- **Behandlung:** Entweder Schema-ehrlich ablehnen (`limit` > Maximum → lenkender
  Fehler) oder — besser — `{items, total, truncated}` als Listenform etablieren.
  Gehört mit B-1 zusammen (eine Listen-Konvention für alle Listen-Tools).

---

## C — Komponierbarkeit & Datenqualität

### C-1 · Nackte Vokabular-URIs überall; Sprach-URIs inkompatibel zum eigenen `lang`-Enum 🟢 (`5b5c594`)

- **Beleg:** `impact_type: …/impact-type/1` (`get_article_history`),
  `type_document: …/resource-type/21` (`get_law_metadata`), `genre: …/legal-resource-genre/100`
  (`get_fga_documents`), `status_uri: …/enforcement-status/3` (`check_in_force`).
  `list_expressions` liefert `…/authority/language/DEU` — während alle anderen Tools
  `lang` als `de|fr|it|en|rm` erwarten; das Ergebnis ist nicht rückführbar.
- **Wirkung:** Pro URI ein zusätzlicher `resolve_vocabulary_label`-Call (Quota!),
  sonst bleibt die Antwort für den Reasoner opak. Die Sprach-URIs kann der Agent
  gar nicht direkt weiterverwenden.
- **Behandlung:** (a) `list_expressions` mappt serverseitig auf die eigenen
  Sprach-Codes; (b) bekannte, kleine Vokabulare (impact-type, enforcement-status)
  als `label` inline mitliefern — der Join ist ein statisches Mapping bzw. ein
  gecachter Lookup, kein zweiter SPARQL-Roundtrip pro Client.

### C-2 · `list_vocabulary`: gültige `scheme_id`s nirgends aufgezählt, kein Suchfilter 🟢 (`4be5113`)

- **Beleg:** Parameter-Description = «Schema-Kennung des Vokabulars» — welche Kennungen
  existieren, steht nirgends («country» war geraten). Deutschland finden hieß: Liste
  blättern (und lief in B-2s stille Kappung).
- **Wirkung:** Das Tool ist nur für Agenten nutzbar, die die Fedlex-Interna schon
  kennen — genau die brauchen es nicht.
- **Behandlung:** Bekannte Schemata in der Description aufzählen (endliche, stabile
  Menge) oder als Enum; zusätzlich `query`-Filterparameter (serverseitiges
  `FILTER(CONTAINS(...))`).

### C-3 · `get_article_history`: identische Duplikat-Einträge, leere Felder 🟢 (`2684af4`)

- **Beleg:** Historie von `art_19` (EnG) → zweimal exakt derselbe Impact-Datensatz
  (gleiche `impact_uri`); `comment: ""`.
- **Wirkung:** Der Agent zählt Änderungen doppelt («Art. 19 wurde zweimal geändert»).
- **Behandlung:** `DISTINCT` bzw. Dedup nach `impact_uri` in JLX-MOD-Query prüfen
  (vermutlich Join-Fanout über Sprachen); leere `comment`-Felder weglassen.
- **Nachtrag (2026-07-03):** Die Verifikation deckte eine zweite Falle auf —
  der Fedlex-WAF blockiert lange Queries mit dem Muster «SELECT … from»
  (HTTP 400 ab ~600 Zeichen; `?from` wie `impactFromLegalResource` zählen).
  Die Impact-Hauptqueries sind seither «from»-frei, die Quell-Erlasse kommen
  aus einer kurzen Zweitquery; Wächter `waf_guard_main_queries_avoid_from`.

### C-4 · Verträge ohne Titel 🟢 (`174cf0f`)

- **Beleg:** `find_treaties`-Hits tragen nur `process_uri` + `signature_date`;
  `get_treaty_info` liefert `title: ""` (Vertrag CH–DE, signiert 2024-04-12 — Titel
  existiert in Fedlex).
- **Wirkung:** N+1-Calls, um zu erfahren, *was* die Treffer sind — und selbst dann
  bleibt der Titel leer. Für die Vertrags-Recherche derzeit der größte Blocker.
- **Behandlung:** Titel-Predicate/Sprachfallback in JLX-TRT-01/02 prüfen
  (vermutlich language-gebundener Titel, DE fehlt → COALESCE über Sprachen);
  Titel in beide Antworten aufnehmen.

### C-5 · `search_law`-Treffer ohne In-Kraft-Status 🟢 (`127d714`)

- **Beleg:** «Energiegesetz» → EnG **1998** (aufgehoben, steht zuerst) und EnG **2016**
  (geltend), **beide SR 730.0**, nichts unterscheidet sie im Treffer.
- **Wirkung:** Die klassische Agenten-Falsch-Wahl: erster Treffer = aufgehobenes
  Recht. Kostet pro Kandidat einen `check_in_force`-Call — oder, schlimmer,
  unterbleibt. Nebenbefund: jeder Hit trägt einen identischen Provenance-Stempel
  (×20 pro Antwort) — reine Token-Redundanz, Top-Level-Provenance genügt.
- **Behandlung:** `in_force`-Flag (oder `status`) pro Hit aus JLX-RES-02 mitliefern;
  geltendes Recht zuerst sortieren; Per-Hit-Provenance entfernen.

### C-6 · `parse_unlinked_ref` parst zu flach 🟢 (`775b7a1`)

- **Beleg:** «Art. 58 Abs. 1 ParlG» → `{kind: "Article", value: "58 Abs. 1 ParlG"}`.
- **Wirkung:** Der Agent muss den Rest selbst parsen — genau die Arbeit, die das Tool
  abnehmen soll. Strukturiert hieße: Artikel 58, Absatz 1, Kürzel «ParlG», plus
  eid-Kandidat (`art_58/para_1`) als Brücke zu `read_element`/`search_law`.
- **Behandlung:** AKN-REF-02 um Absatz/Kürzel-Extraktion erweitern; Ausgabefelder
  `article`, `paragraph`, `act_abbreviation`, `eid_candidate` (weiterhin hint).

### C-7 · Fehler-Hints nennen das Folge-Tool nicht 🟢 (`432ed88`)

- **Beleg:** Unbekanntes ELI/eid → «Pruefe ELI/Stichtag oder nutze ein Suchtool.»
  `get_oc_act` mit AS-ELI (plausible Verwechslung: das Tool handelt *von* der AS)
  → «existiert nicht», statt «erwartet Werk-ELI `eli/cc/…`».
- **Wirkung:** Selbstkorrektur braucht einen zweiten Anlauf mehr als nötig. Der
  Kontrast zeigt es: der Argument-Fehler («`label` fehlt») ist feld-genau und sofort
  behebbar — die NotFound-Hints sind es nicht.
- **Behandlung:** Hints konkretisieren: bei eid-Fehlern → `get_structure`/`search_text`
  für *dieses* ELI nennen; bei ELI-Fehlern → `search_law`/`resolve_sr_number`; bei
  ELI-*Typ*-Fehlern (oc statt cc) → erwartete Form benennen. Nebenbei: Umlaute in
  Hint-Texten («Pruefe» → «Prüfe»).

### C-8 · Output-Feldnamen ≠ Input-Parameter der Folge-Tools 🟢 (`0c38627`, via Descriptions statt Breaking Rename)

- **Beleg:** `find_treaties` → `hits[].process_uri`, aber `get_treaty_info` will
  `uri`; `get_drafts` → `uri`, aber `get_consultations` will `draft_uri`;
  `get_consultation_documents` will `consultation_uri`.
- **Wirkung:** Jede Kette erfordert Raten oder Schema-Lektüre; in der Session zwei
  vermeidbare Fehlversuche.
- **Behandlung:** Günstigste Lösung ohne Breaking Change: Parameter-Descriptions
  benennen die Herkunft («`uri`: die `process_uri` aus `find_treaties`»). Ambitionierter:
  Feldnamen harmonisieren (Output-Feld heißt wie der Parameter des Folge-Tools).

### C-9 · Quota für den Agenten unsichtbar ⚪ (verworfen 2026-07-02)

- **Beleg:** Keine RateLimit-Header, kein Quota-Status-Tool; der Agent erfährt sein
  Budget erst beim in-band Quota-Fehler.
- **Wirkung:** Gering — der graceful Fehler existiert und trägt Retry-Semantik.
- **Behandlung:** Verworfen: kein eigenes Tool (Quota gehört nicht in die
  LLM-Schicht, ADR-002-Nähe), und der graceful in-band Quota-Fehler trägt
  bereits `retry_after_ms` — der Agent erfährt beim einzigen relevanten
  Ereignis alles Nötige. `X-RateLimit-*`-Header bleiben eine Option auf
  HTTP-Ebene, falls je ein Host sie auswertet.

---

## Reihenfolge-Empfehlung

**A-1 → A-3 → A-2** (Sichtbarkeitsschicht: zwei davon sind Kleinstaufwand, A-2 braucht
ADR-011) — dann **C-3/C-4** (Datenqualität der neuen Tranche, SPARQL-seitig) — dann
**B-1/B-2** als gemeinsame Listen-/Budget-Konvention (löst 67 P-5 mit ab) — dann
C-1/C-2/C-5–C-8 als Ergonomie-Welle. *(So umgesetzt am 2026-07-02.)*

---

## F — MCP-Explorer-Lauf 2026-07-06 (unvoreingenommene 3-Phasen-Evaluation)

> **Quelle:** Explorativer Benchmark `benchmarks/mcp-explorer` (Repo mindful.bio),
> Lauf `runs/2026-07-06/` — vier frische Phase-1-Explorer, je ein frischer Phase-2-
> (Zusammenspiel mit mcp-fedlex-semantic) und Phase-3-Agent (Ketten), 296 protokollierte
> Aufrufe, Navigator-JWT gegen mcp-fedlex.ch. Vollständiger Bericht inkl. Log-Referenzen
> (`log:<server>:<n>` → `calls-*.jsonl`): `runs/2026-07-06/REPORT.md`; Scorecards,
> Positivbefunde und Messwerte dort. Hier nur die Reader-seitigen Findings; die
> semantic-seitigen (F7, F8, F14, F15, F17, F20, F25, F30, F31) leben im Register des
> mcp-fedlex-semantic-Repos. Nummerierung folgt dem Berichts-Kapitel 6.

| Nr | Grad | Finding (ein Satz) | Repro | Status |
|---|---|---|---|---|
| F-1 | Blocker | search_law findet das geltende DSG (eli/cc/2022/491) unter keinem Titel-Stichwort — alle Treffer aufgehoben, entgegen «geltendes Recht steht zuerst» | `search_law {"query":"Datenschutzgesetz"}` | 🟢 historicalLegalId OPTIONAL; nDSG live auffindbar (sr_number dann None, via get_law_metadata aufloesbar) |
| F-2 | Reibung | Textzugriff auf aufgehobenes aDSG scheitert als «Ressource existiert nicht» + zirkulärer Hint, obwohl list_versions/explore_node die Konsolidierung belegen | `read_article {"eli":"eli/cc/1993/1945_1945_1945","eid":"art_8","as_of":"2014-01-01"}` | 🟢 NotFound benennt jetzt «keine konsolidierte XML-Fassung … Stichtag … Sprache»; Hint nennt list_versions/list_expressions/PDF-Aera statt Kreis-Verweis. (Text-Zugriff selbst bleibt Daten-Realitaet: XML erst ab ~2021) |
| F-3 | Reibung | `in_force`/`in_force_status` in search_law-/resolve_sr_number-Treffern ignoriert `as_of` und widerspricht check_in_force — Disambiguierung wählt am Stichtag das falsche Gesetz | `resolve_sr_number {"sr_number":"235.1","as_of":"2020-06-01"}` | 🟢 in_force stichtagstreu aus Datumsfeldern (J3.2); ohne Daten + historischem as_of ehrlich weggelassen |
| F-4 | Reibung | search_law ist Substring-/Phrasensuche: «OR»→Rheinschiffe, «ArG»→Argentinien, obwohl parse_unlinked_ref Kürzel explizit dorthin verweist | `search_law {"query":"OR","limit":5}` | 🟢 Kuerzel-Vorabfrage exakt auf jolux:titleShort (live: OR/ArG/ZGB/DSG in ~0.3 s), Treffer ranken vor Substring; Volksnamen via titleAlternative |
| F-5 | Reibung | Leere Trefferlisten ohne Reformulierungs-/Umleitungs-Hinweis; keine Umlaut-Normalisierung («ueber» 0 vs. «über» 1 Treffer) | `search_law {"query":"Bundesgesetz ueber den Datenschutz"}` | 🟢 Umlaut-Variante als ODER-Zweig (live verifiziert); 0-Treffer-Antworten tragen jetzt einen hint mit Auswegen |
| F-6 | Reibung | Default-Stichtag ist der Boot-Tag des Pods statt «heute» (real: Vortag), divergent zum semantic-Server, entgegen instructions | Aufruf ohne `as_of`, provenance.valid_as_of prüfen | 🟢 `TemporalResolver::swiss_today()`: Anfrage-Tag Europe/Zurich statt Boot-Tag |
| F-9 | Reibung | search_law: eponymes Gesetz fehlt in Top-N (GlG), truncated=true ohne offset-Parameter — Rest unerreichbar | `search_law {"query":"Gleichstellung","limit":8}` | 🟡 offset-Parameter + offset_applied nachgeruestet (truncated ist keine Sackgasse mehr); eponymes Ranking weiter datengetrieben via Kuerzel/titleAlternative (F-4) statt Score |
| F-10 | Reibung | get_law_metadata liefert für valide, nicht existierende ELIs ein Null-Objekt mit kind=norm statt Fehler | `get_law_metadata {"eli":"eli/cc/9999/99999"}` | 🟢 CA-Typ als Pflicht-Pattern: 0 Bindings -> gracefuler NotFound statt Null-Objekt mit kind=norm (live: bogus ELI -> 0 Bindings) |
| F-11 | Reibung | get_law_metadata: sr_number=null (obwohl via SR gefunden), Abkürzung/Status fehlen | `get_law_metadata {"eli":"eli/cc/2022/491"}` | 🟢 sr_number mit Taxonomie-Fallback (nDSG live: 235.1), neu abbreviation (titleShort: DSG) + current_status_uri/-label («In Kraft») |
| F-12 | Reibung | URI-Roundtrip bricht: Tools emittieren https-URIs, eli-Params verlangen `eli/`-Kurzform; ungesplittete chunk_ids ohne Split-Hinweis | `read_article {"eli":"https://fedlex.data.admin.ch/eli/cc/2022/491",…}` | 🟢 require_eli strippt volle fedlex-URLs zentral; chunk_ids bekommen ein konkretes Split-Rezept (eli + eid) statt Prefix-Fehler |
| F-13 | Reibung | Fehlende Sprachfassung (rm) als «Ressource existiert nicht» fehlattributiert; Hint ohne lang/list_expressions | `read_article {…,"lang":"rm"}` | 🟢 gleiche Fehlerform wie F-2: Fassungs-/Sprach-Fehlen wird benannt, Hint zeigt list_expressions |
| F-16 | Reibung | Typfehler als «fehlt» gemeldet («'eid' (string) fehlt» bei eid:21) | `read_article {…,"eid":21}` | 🟢 require_str zentral («muss ein String sein, nicht Zahl»); lang typ-streng statt still-Deutsch. Semantic-Pendant separat (dortiges Register) |
| F-18 | Reibung | Unbekannte scheme_id → stillschweigend leere Liste (isError:false) statt {error, hint}; Schemes nicht enumerierbar | `list_vocabulary {"scheme_id":"bogus"}` | 🟢 leeres Ergebnis ohne query -> NotFound mit Aufzaehlung der bekannten Schemes (eine Quelle fuer Schema-Text und Fehler); mit query bleibt leer eine legitime Liste |
| F-19 | Reibung | Trefferliste: dieselbe ELI doppelt mit widersprüchlichem in_force; Entwürfe ohne in_force-Feld untergemischt | `search_law {"query":"Covid-19","as_of":"2021-02-01"}` | 🟢 Dedup pro ELI macht widerspruechliche Doppel-Treffer unmoeglich (13c493d); Treffer ohne in_force sortieren sichtbar in die Mitte — Entwurfs-Vermischung ist Daten-Realitaet, am Flag erkennbar |
| F-21 | Schönheit | tools/list 91 KB, ~25 % byte-identisches Duplikat (Legacy-`schema`-Doppel-Emit) | `tools/list` | 🔴 |
| F-22 | Schönheit | 38 Parameter ohne description; get_references.limit bricht 20/50-Muster; Enum-Validierung uneinheitlich (depth fällt still auf Default) | Schema-Inspektion | 🔴 |
| F-23 | Schönheit | Keine Protokollversions-Verhandlung: jede angefragte Version → 2025-11-25 | `initialize {"protocolVersion":"1999-01-01"}` | 🔴 |
| F-24 | Schönheit | Auth-Fehler-HTTP-Status divergiert je Transportpfad (rpc 200 in-band, mcp 401+WWW-Authenticate) | `tools/list` ohne Token auf beiden Pfaden | 🔴 |
| F-26 | Schönheit | Fehler-Hint bei SR-Nummer-als-eli generisch statt resolve_sr_number zu nennen | `read_article {"eli":"151.1",…}` | 🟢 eigener Hint fuer «expected prefix eli/»: nennt resolve_sr_number und den URL-Strip |
| F-27 | Schönheit | in_force_status als nackte Vokabular-URI in resolve_sr_number (check_in_force hat current_status_label) | `resolve_sr_number {"sr_number":"235.1"}` | 🟢 in_force_status_label direkt gejoint (wie current_status_label in check_in_force) |
| F-28 | Schönheit | Zwei dokumentierte eid-Normalformen (read_element vs. get_article_history) | Schema-Vergleich | 🔴 |
| F-29 | Schönheit | Absurde Zukunfts-Stichtage (2999) kommentarlos als kind=norm beglaubigt | `check_in_force {…,"as_of":"2999-12-31"}` | 🟢 check_in_force kennzeichnet Zukunfts-Stichtage mit future_as_of: true (Projektion, kein beglaubigter Zustand) |
| F-32 | Schönheit | Verbrückung einseitig: kein Reader-Hinweis auf die semantische Alternative bei 0 Treffern | `search_law` mit Laienfrage | 🟢 Reader: 0-Treffer-hint verweist auf semantic_search (3e90d07); semantic nennt den Partner als mcp-fedlex-reader (4918d5e) |
| F-33 | Schönheit | get_citations englisch ohne lang-Param; Caveat «leere Liste ≠ nie geändert» fehlt in leerer Antwort; JLX-/AKN-Codes unerklärt | diverse | 🔴 |
| F-34 | Schönheit | search_law p50 ≈ 800 ms — langsamstes Tool ausgerechnet in der Recovery-Schleife | Latenz-Logs | 🔴 |

---

## V — Verifikations-Zweitlauf 2026-07-06 (neue Findings) — Abarbeitung 2026-07-07

> Quelle: `benchmarks/mcp-explorer` `runs/2026-07-06-verify/REPORT.md` §10 (Bilanz Iteration 1:
> 13 behoben, 10 verbessert). Semantic-seitige Fixes referenzieren das Sibling-Repo.

| Nr | Finding (ein Satz) | Status |
|---|---|---|
| V-1 | truncated:false trotz Recall-Lücke — der F-19-Dedup schrumpfte NACH dem SPARQL-LIMIT | 🟢 Overfetch limit×2, Kappung nach Dedup (d21777f) |
| V-2 | Stub-ELIs (eli/cc/2020/2930_cc) als vollwertige Treffer | 🟢 stub:true-Marker (weder Status noch Datum), sortiert ans Gruppen-Ende (d21777f) |
| V-3 | Null-Daten als kind=norm (check_in_force lauter Nulls → in_force:false) | 🟢 no_enforcement_data:true kennzeichnet «keine Daten» (d21777f) |
| V-4 | Cloudflare-BIC bannt Python-urllib/libwww-perl vor dem ersten MCP-Byte | 🟢 Configuration Rule «BIC off» fuer /rpc,/mcp,/sse deployt 2026-07-07; verifiziert (Bot-UA /rpc→200, /mcp→401, Docs bleibt 403). Runbook k3-infra b191fb9 |
| V-5 | Nonsens-Queries erzeugen unauffällige Scores | 🟡 Mitigation: «NICHT kalibriert» in instructions+Schema (semantic c1edb0e); echte Detektion braucht Goldset-Experiment |
| V-6 | exclude_source_eli filterte nach dem Retrieval → falsche 0-Treffer | 🟢 serverseitig via Qdrant must_not (semantic 437f83d) |
| V-7 | Stille Koersion optionaler Argumente (as_of:Zahl→heute; rerank:"yes"→false; top_k:-5→20) | 🟢 beidseitig typ-streng (82583d6, semantic e385a2c) |
| V-8 | Semantic-Kaltstart ~7 s unkommuniziert | 🟢 Warmup beim Boot hinter der startupProbe (semantic c1edb0e) |
| V-9 | Sprach-/Korpus-Coverage unsichtbar | 🟡 language:de in index_info (semantic c1edb0e); ELI-Zähler/Frische bleiben offen (=F-8) |
| V-10 | Semantic-Treffer ohne Erlass-Titel → Metadata-Aufruf je ELI | 🟢 title im Hit (Qdrant-Payload; Bestandsdaten nach Re-Ingest) (semantic 5aafba8) |
| V-11 | parse_unlinked_ref verwirft lit./Bst./Ziff. stillschweigend | 🟢 sub_reference erhält die Feinreferenz roh (ae4c2f1) |
| V-12 | resolve_sr_number: dieselbe ELI doppelt mit widersprüchlichen Labels (SR 818.102) | 🟢 Dedup pro ELI + Feld-Merge (e480a42) |
| V-20 | resolve_consolidation_at: nacktes not-found statt Fassungs-Diagnose | 🟢 gleiche differenzierte Meldung wie F-2 (ae4c2f1) |
| V-24 | index_info-outputSchema beschrieb hits-Felder, die es nie gibt | 🟢 eigenes Schema (semantic c1edb0e) |
| V-28 | get_article_history verschweigt Einfügung 2021 (OR 329g); list_versions ohne XML-Verfügbarkeit | 🔴 Untersuchungs-Ticket: Impact-Query-Lücke vs. Daten-Realität live klären |
