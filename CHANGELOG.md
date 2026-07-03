# Changelog

Alle nennenswerten Änderungen an diesem Projekt werden hier festgehalten.

Das Format orientiert sich an [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
und das Projekt folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

- **RF-5: Redis-Passwort stand im Klartext im Startup-Log.** `REDIS_URL` wird
  nur noch redigiert geloggt (`RedactedRedisUrl` in `fedlex-store`: Userinfo →
  `***`, Rohwert wird nie gespeichert); Regressionstest gegen das
  `:<pw>@`-Muster.

- **`get_citations` lief bei Erlassen mit realem Zitationsnetz (DSG) in den
  15-s-Timeout (RF-6):** Der `FILTER(STRSTARTS(…))`-Full-Scan über den
  gesamten Zitationsgraphen ist durch das Zwei-Query-Muster aus JLX-IMP-01
  ersetzt — «from»-freie Stichtags-Auflösung der Fassung, danach kurze,
  exakt gebundene Zweitqueries (< 0,5 s live, Wächter
  `waf_guard_citation_queries`). Ergebnisse sind neu nach Quellgesetz
  dedupliziert (J7.4) und tragen Erlass-URIs statt `/text`-Formen.
- **`resolve_sr_number` fand geltendes Recht ohne SR-Literal nicht (RF-6):**
  Das nDSG (`eli/cc/2022/491`, SR 235.1) trägt kein `historicalLegalId`
  mehr — die Auflösung läuft neu zusätzlich über die Systematik-Taxonomie
  (`skos:notation`, typisiert) und sortiert geltendes Recht zuerst.
- **`check_in_force` vermischte zwei Zeitbezüge (RF-6):** `status_uri`/
  `status_label` heissen neu `current_status_uri`/`current_status_label`
  (serde-Alias für Alt-Payloads) und sind als **heutiger** Vokabular-Status
  ausgewiesen; stichtagsbezogen ist allein `in_force`.
- **Batch-Requests melden neu klar** «JSON-RPC batching is not supported
  (removed in MCP 2025-06-18)» statt des rohen serde-Parse-Fehlers (RF-6).

### Added

- **CORS-Preflight auf `/mcp` (SOTA-T3):** `OPTIONS /mcp` + CORS-Echo für
  Origins der `MCP_ALLOWED_ORIGINS`-Allowlist — browserbasierte MCP-Clients
  können verbinden; fremde Origins bleiben 403, ohne Allowlist bleibt alles
  fail-closed.
- **HTTP 401 auf `/mcp` bei Auth-Fehlern (SOTA-T4):** `WWW-Authenticate:
  Bearer`, JSON-RPC-Fehlerhülle im Body unverändert. Legacy-`/rpc` bleibt
  bewusst bei 200+JSON für Alt-Clients (Kompat-Wächter-Test).

- **Provenance trägt optional `date_applicability`** — das Stand-Datum der
  tatsächlich aufgelösten Fassung (RF-6). Damit ist ein künftiger Stichtag
  (`as_of=2035-01-01`) nicht mehr stillschweigend als bestätigte Fassung
  lesbar; künftige Stichtage bleiben bewusst zulässig (Fedlex führt
  beschlossene künftige Fassungen). Additiv und abwärtskompatibel
  (ADR-004-Hülle unverändert).

### Removed

- **Verdrahtungsruinen entfernt (67 §W-1/W-2):** Die fünf vorgebauten, nie an
  den Live-Pfad angeschlossenen Reader-Module sind gelöscht — `xml_engine`
  (Stub-Parser; Single-Flight-Idee lebt produktiv im Fetcher, H-5; das
  `diff_to_markdown`-Duplikat zu `compare_versions` entfällt damit),
  `sandbox` (Muster lebt produktiv als `spawn_blocking` im Fetcher, H-6),
  `warmup`, `lod_gateway` und `semantic_client` (Git vergisst nichts; die
  reservierten RBAC-Pools `LodFederation`/`Workspace` bleiben dokumentiert,
  s. 90_AUTH_AND_ROLES §3). Grundsatz: kein exportiertes Modul ohne
  produktiven Nutzer.

### Added

- **Top-Level-`description` in `tools/list` (68 §A-1):** Jeder Tool-Eintrag
  trägt seine Beschreibung jetzt zusätzlich im MCP-Standardfeld `description` —
  dem Feld, das Hosts dem Modell präsentieren. Zuvor stand der Text nur in
  `inputSchema.description`, womit alle 40 Tools in Standard-Hosts ohne
  Erklärung erschienen. Abnahme: `protocol_baseline` verlangt nicht-leere
  Top-Level-Description == Schema-Text für jeden Eintrag.
- **`instructions` im `initialize`-Result (68 §A-3):** Der Server liefert dem
  Host eine kompakte Hausordnung für das Modell (norm vs. hint, typischer
  Recherche-Ablauf, ELI-/eid-Formen, Stichtag-Semantik, Fehlerform) — der
  Spec-Kanal, über den ein MCP-Server dem Agenten seine Semantik VOR der
  ersten Tool-Wahl erklärt. Abnahme: Baseline + Transport-Test verlangen die
  Kernbegriffe im Feld.
- **Ketten-Herkunft in den Parameter-Beschreibungen (68 §C-8):** Parameter,
  die aus der Antwort eines anderen Tools stammen, nennen jetzt Quelle und
  Feldname exakt (`get_treaty_info.uri` ← `find_treaties.process_uri`;
  `get_consultations.draft_uri` ← `get_drafts.uri`;
  `get_outgoing_impacts.eli` ← `get_oc_act.oc_uri`; `resolve_vocabulary_label`
  akzeptiert jede Vokabular-URI aus früheren Antworten). In der
  Dogfooding-Session kosteten die stummen Parameternamen zwei Fehlversuche.
- **Fehler-Hints nennen das Folge-Tool (68 §C-7):** eId nicht gefunden →
  «hole die Gliederung mit `get_structure` oder finde die Stelle mit
  `search_text`»; ELI nicht gefunden → «finde den Erlass mit `search_law`
  bzw. `resolve_sr_number`» (statt «nutze ein Suchtool»). Die plausible
  ELI-Familien-Verwechslung wird früh abgefangen: `get_oc_act` mit AS-ELI
  bzw. `get_memorial` mit Werk-ELI liefern einen lenkenden Fehler, der die
  erwartete Form und das Folge-Tool nennt, statt in ein nacktes NotFound zu
  laufen (live passiert).
- **`parse_unlinked_ref` zerlegt strukturiert (68 §C-6):** «Art. 58 Abs. 1
  ParlG» liefert jetzt `article: "58"`, `paragraph: "1"`,
  `act_abbreviation: "ParlG"` und `eid_candidate: "art_58/para_1"` — die
  direkten Brücken zu `read_element` (eid) und `search_law` (Kürzel).
  Vorher bekam der Agent nur `{Article, "58 Abs. 1 ParlG"}` und musste
  selbst weiterparsen — genau die Arbeit, die das Tool abnehmen soll.
  Konservativ: Unsicheres bleibt `None`, der Rohtext immer in `value`.
- **Geltungs-Flag in den Such-Treffern (68 §C-5):** `search_law`- und
  `resolve_sr_number`-Treffer tragen `in_force` (abgeleitet aus
  `jolux:inForceStatus`; fehlt der Status, ehrlich `null` → `check_in_force`
  entscheidet über die Datumsfelder). `search_law` sortiert geltendes Recht
  zuerst — live stand das **aufgehobene** EnG 1998 vor dem geltenden EnG 2016
  (beide SR 730.0), die klassische Agenten-Falsch-Wahl. Die redundante
  Per-Hit-Provenance (N identische Stempel pro Antwort) entfällt; die
  Antwort-Hülle trägt die Hinweis-Provenance.
- **`list_vocabulary` findbar gemacht (68 §C-2):** Neuer `query`-Parameter
  filtert serverseitig case-insensitiv über die Labels aller Sprachen
  (`scheme_id=country, query=Deutschland` → 1 Treffer statt Liste blättern
  und in die Kappung laufen; live verifiziert). Die gültigen Schema-
  Kennungen sind jetzt in der Parameter-Description aufgezählt — vorher
  stand dort nur «Schema-Kennung des Vokabulars», ohne dass irgendwo
  dokumentiert war, welche existieren.
- **Vokabular-Labels direkt in den Antworten (68 §C-1):** Die opaken
  Vokabular-URIs (`impact-type/1`, `enforcement-status/3`,
  `resource-type/21`, Genre-URIs) tragen jetzt ihr deutsches Label inline —
  per `skos:prefLabel`-Join in derselben SPARQL-Query, kein zweiter
  Roundtrip: `impact_type_label` (Historie), `status_label`
  (`check_in_force`), `type_document_label` (`get_law_metadata`),
  `genre_label` (`get_oc_act`/`get_fga_documents`). `list_expressions`
  liefert statt EU-Vokabular-URIs die eigenen Sprach-Codes (`de|fr|it|en|rm`)
  — direkt als `lang`-Argument weiterverwendbar; fremde URIs werden roh
  durchgereicht. Query-Formen live gegen Fedlex verifiziert.
- **Truncation-Signale statt stiller Kappung (68 §B-2):** Alle limit-gekappten
  Listen tragen jetzt `truncated` + `limit_applied` (`search_law`,
  `find_related_topic`, `find_treaties`, `list_vocabulary`, `explore_node`);
  `search_text` liefert `{hits, total, truncated}` — `total` zählt alle
  Fundstellen, auch jenseits von `max_hits`. Vorher: `limit: 500` →
  wortlos 50 Ergebnisse; der Agent hielt das Fenster für die Gesamtheit
  (live an der Länderliste beobachtet — wäre «Deutschland» nicht zufällig
  in den ersten 50 gewesen, wäre die Antwort «existiert nicht» gewesen).
  Abnahme: Kappungs-Tests in `discovery.rs`, `tools.rs`, `text.rs`.
- **Antwort-Budgets für die schweren Navigations-Tools (68 §B-1, löst 67 §P-5
  für den Live-Pfad):** Live-Messung am EnG: `read_document` 210 KB,
  `get_structure` 95 KB, `get_references` 82 KB — ein Aufruf konnte das
  Kontext-Budget eines Agenten sprengen. Jetzt: `get_structure` liefert per
  Default das **Artikel-Skelett** (`depth=article`; `depth=full` für den
  ganzen Baum), leere `children`/`null`-Felder werden nicht mehr
  serialisiert; `read_document` bekommt ein Zeichen-Budget (`max_chars`,
  Default 120 000; `0` = unbegrenzt) mit ehrlichem `truncated`-Signal und
  Fortsetzung über `offset`/`next_offset`; `get_references` liefert die
  Listenform `{references, total, truncated}` mit `limit`/`offset`
  (Default 200). Abnahme: Budget-/Fortsetzungs-Test (lückenloses Stitching),
  Skeleton-vs-full-Test, Listenform-Test.
- **Stichtag als Tool-Argument (68 §A-2, ADR-011):** `as_of` (JJJJ-MM-TT) wird
  jetzt auch in den `arguments` gelesen — der einzige Kanal, den ein Modell
  über einen Standard-MCP-Host erreicht. Vorher wurde es dort stillschweigend
  ignoriert (heutiges Recht statt historischer Anfrage); `check_in_force` war
  zum historischen Stichtag gar nicht befragbar. Vorrang: `params.as_of`
  (Host-gepinnt) > `arguments.as_of`; jedes Tool annonciert das optionale
  Property zentral im Schema; die Provenance stempelt weiterhin serverseitig
  das effektiv verwendete Datum (ADR-004 unberührt). Abnahme: Transport-Tests
  (Kanal, Vorrang, in-band Error) + Baseline (Annonce, nie required).
- **Upstream-Timeouts (67 §H-1):** Alle ausgehenden HTTP-Clients (Fedlex-SPARQL,
  AKN-Filestore, JWKS-Abruf) tragen zwingend Connect- (3 s) und Gesamt-Timeout
  (15 s), konfigurierbar über `MCP_UPSTREAM_CONNECT_TIMEOUT_MS` /
  `MCP_UPSTREAM_TIMEOUT_MS` (70_CONFIG §1a). Neu `fedlex_bridge::HttpTimeouts`;
  `HttpSparqlClient::new/fedlex` und `HttpXmlSource::new` sind dadurch fallible
  (sauberer Startabbruch statt reqwest-Panik bei TLS-Init-Fehlern). Abnahme:
  `tests/http_timeouts.rs` beweist den Abbruch gegen einen hängenden Upstream.
- **`GET /metrics` (Prometheus, 67 §O-2):** Betriebszähler für Rate/Fehler/
  Latenz je Tool (`mcp_tool_calls_total{tool,outcome}`,
  `mcp_tool_call_duration_ms{tool}`), Lastschutz (`mcp_requests_shed_total`,
  `mcp_requests_timed_out_total`), Quota-Degradierung
  (`mcp_quota_fallback_total`) und Breaker-Short-Circuits
  (`mcp_upstream_short_circuit_total{upstream}`). Labels sind bounded (nie
  Argumente/Mandanten — PII-Disziplin wie im Audit-Log); der Endpoint liegt
  ausserhalb des Lastschutzes und wird am Ingress nicht öffentlich geroutet.
- **Strukturiertes Logging (67 §O-1):** `tracing` ersetzt alle 14
  `println!`/`eprintln!` — Level via `RUST_LOG`, Format via `MCP_LOG_FORMAT`
  (`text` lokal, `json` für K8s). Die Audit-Zeile pro Tool-Call läuft als
  `target: "audit"` weiter durch den PII-Scrubber, hängt aber nicht mehr am
  globalen stdout-Lock auf dem Request-Pfad. CLAUDE.md-Korrektur:
  `fedlex-telemetry` ist der PII-Scrubber, kein „Tracing-Layer" (67 §O-3).
- **Tool-Annotations (67 §P-2):** Jeder `tools/list`-Eintrag trägt jetzt
  `annotations` mit `readOnlyHint: true`, `idempotentHint: true`,
  `openWorldHint: false` — zentral gesetzt, weil der Reader als
  CQRS-Leseseite ausnahmslos read-only ist; der Baseline-Test verriegelt
  die Annotation für jedes Tool.
- **Readiness von Fedlex entkoppelt (67 §H-8):** `/readyz` kennt jetzt
  kritische (Redis) und **informative** Prüfungen (Fedlex, mit 2-s-Deckel):
  Ein Fedlex-Ausfall erscheint als `degraded` im Body, nimmt die Pods aber
  nicht mehr aus dem Load-Balancer — Cache und lokale Navigation bedienen
  weiter. Abnahme: `degraded_informational_probe_keeps_readyz_green`.
- **Graceful Shutdown (67 §H-7):** SIGTERM (Kubernetes) und Ctrl-C beenden
  den Server geordnet — In-Flight-Requests werden zu Ende bedient (Drain),
  neue Verbindungen nicht mehr angenommen. Rolling-Deploys produzieren keine
  502er mehr. Abnahme: `graceful_shutdown_drains_in_flight_request`.
- **XML-Pfad entkoppelt und begrenzt (67 §H-6):** Der CPU-gebundene
  AKN-Parse (roxmltree, 1–10 MB) läuft in `spawn_blocking` statt auf dem
  Runtime-Worker; der XML-Download hat eine Obergrenze (`MCP_XML_MAX_BYTES`,
  Default 32 MB), die beim angekündigten `Content-Length` und als Kappung im
  Stream greift. Abnahme: drei Tests in `tests/xml_limits.rs`.
- **Manifestations-Cache: Single-Flight + Byte-Budget (67 §H-5):** N parallele
  Misses auf dieselbe Manifestation lösen jetzt genau **einen** Download+Parse
  aus (`try_get_with`; Fehler werden nicht gecacht und strukturerhaltend an
  alle Wartenden geteilt — die Fehler-Enums sind dafür `Clone`). Das
  Cache-Budget ist neu ein **Byte-Budget** über die XML-Größen
  (`MCP_FETCHER_CACHE_MAX_BYTES`, Default 256 MB, TinyLFU-Eviction) statt
  einer Eintragszahl (64 × bis zu 10 MB waren unbegrenzt). Abnahme:
  `parallel_misses_trigger_exactly_one_fetch`, `failed_fetch_is_not_cached`.
- **Redis-Härtung (67 §H-4):** Der Token-Bucket nutzt eine geteilte,
  selbstheilende Verbindung (`ConnectionManager`, Lazy-Init) statt pro Aufruf
  neu zu verbinden — mit mTLS war das ein TLS-Handshake **pro Request**. Jede
  Redis-Operation steht unter einer Zeitgrenze (`MCP_REDIS_OP_TIMEOUT_MS`,
  Default 2 s): ein *hängendes* Redis fällt jetzt fail-closed in den
  Fallback-Bucket statt den Request-Pfad zu blockieren. Abnahme:
  `hanging_redis_hits_op_timeout_instead_of_blocking`.
- **Circuit Breaker auf den Live-Pfaden (67 §H-3):** Der vorgebaute, bisher
  unverdrahtete `CircuitBreaker` liegt jetzt als Decorator um beide
  Transport-Traits (`resilience.rs`): ein gemeinsamer Breaker für alle
  SPARQL-Pfade (Fetcher, Discovery, Metadaten teilen die Fehlerzähler), ein
  eigener für den AKN-Filestore. Nach 5 Fehlern in Folge scheitern Aufrufe
  30 s lang sofort mit lenkender Meldung (Fail-Fast statt Task-Stau), dann
  prüft ein Probe-Aufruf die Genesung. Die Readiness-Probe bleibt bewusst am
  rohen Client. Abnahme: drei Decorator-Tests (Short-Circuit ohne
  Endpunkt-Berührung, Erfolgsdurchfluss).
- **Lastschutz am Router (67 §H-2):** Die MCP-Routen laufen hinter
  Timeout- (`MCP_REQUEST_TIMEOUT_MS`, Default 30 s) und Concurrency-Layer
  (`MCP_MAX_CONCURRENT_REQUESTS`, Default 256, Load-Shedding). Zeitüberschreitung
  → HTTP 504, Überlast → HTTP 503 + `Retry-After`, beide mit lenkendem
  `{error, hint}`-Body. Health-Probes bleiben bewusst ausserhalb des Schutzes
  (Pod darf unter Last nicht von Kubernetes gekillt werden). Abnahme: drei
  Tests in `app.rs` (Timeout kappt, Überlast shedded, `/livez` unter Volllast).

### Fixed

- **Fedlex-WAF blockiert «from»-Queries (live diagnostiziert 2026-07-03):**
  Der WAF vor dem SPARQL-Endpoint weist Queries mit dem SQL-Injection-Muster
  «SELECT … from» ab HTTP-400 zurück, sobald sie ~600 Zeichen überschreiten —
  getroffen hat es `get_impacts`/`get_article_history` (Variable `?from` +
  Prädikat `impactFromLegalResource`), nachdem der C-1-Label-Join die Query
  über die Schwelle hob. Fix: Die Hauptqueries sind jetzt komplett
  «from»-frei; die Quell-Erlasse holt eine zweite, kurze Query (unter der
  Schwelle, live verifiziert: Art.-19-Historie korrekt, alle 114
  EnG-Impacts mit Quelle); `OUTGOING_Q` nutzt `?src`. Regressions-Wächter:
  `waf_guard_main_queries_avoid_from`.
- **Vertragstitel (68 §C-4):** `get_treaty_info` lieferte leere Titel —
  `jolux:titleTreaty` ist sprach-getaggt und Fedlex trägt teils **leere**
  Sprachvarianten (live: `@en=""` am CH–DE-Vertrag 2024/0088); die Query nahm
  blind die erste Zeile. Jetzt: Leerstring-Filter in der Query, sprach-
  präferente Wahl (Wunschsprache → de → fr → it → en → rm), neuer optionaler
  `lang`-Parameter an beiden Vertrags-Tools. `find_treaties`-Treffer tragen
  den Titel jetzt direkt (vorher nur Prozess-URI + Datum → N+1-Calls pro
  Treffer). Live gegen Fedlex verifiziert; Abnahme: 3 neue Tests in
  `treaties.rs`.
- **Impact-Duplikate & Leer-Kommentare (68 §C-3):** `get_article_history` und
  `get_impacts` lieferten inhaltsgleiche Zeilen doppelt (Join-Fanout über die
  gefilterte, aber nicht projizierte `?target`-Variable — live beobachtet an
  Art. 19 EnG: «zweimal geändert», tatsächlich einmal). Alle drei
  Impact-Queries dedupen jetzt mit `SELECT DISTINCT`, dazu defensive,
  reihenfolge-erhaltende Dedup in Rust; leere `comment`-Literale werden zu
  `null` statt `""`. Abnahme: Duplikat-Fixture-Test in `impacts.rs`.

## [0.2.0] - 2026-06-21

MCP-Protokoll-Upgrade auf die stabile Spec-Revision `2025-11-25` (ADR-008
abgeschlossen). Der Konsumenten-Vertrag bleibt additiv: bestehende Clients
(ansV, syllogismus-fedlex) laufen unverändert weiter.

### Added

- **Streamable-HTTP-Transport (`POST /mcp`)** additiv neben `/rpc` (ADR-008 §B,
  Runbook Phase 3). Der neue Endpoint setzt die zwei Transport-Wächter der
  Revision `2025-11-25` **vor jeder Arbeit** durch:
  - **`Origin`-Prüfung → HTTP 403** bei einem `Origin` ausserhalb der
    Allowlist `MCP_ALLOWED_ORIGINS` (DNS-Rebinding-Schutz, fail-closed);
  - **`MCP-Protocol-Version`-Header → HTTP 400** bei gesetztem, nicht
    unterstütztem Wert.
  Fehlende Header bleiben rückwärtskompatibel (Server-zu-Server-Clients ohne
  `Origin`/Header unverändert erlaubt). Klassifikatoren `classify_origin` /
  `classify_protocol_header` in `protocol.rs` mit Unit-Tests; sechs HTTP-Tests
  für `/mcp` (403/allowed/no-origin/400/supported/202-notification).

### Changed

- **`tools/list` liefert nun `inputSchema` zusätzlich zu `schema`** (additiver
  Doppel-Output, gleicher Wert). Der MCP-Standard verlangt über alle Revisionen
  `inputSchema`; der Legacy-Schlüssel `schema` bleibt übergangsweise erhalten, bis
  beide Konsumenten umgestellt sind, und fällt erst in Phase 9 (ADR-008 §B-5,
  Runbook 55 Schritt 7.2a). Der Baseline-Test fixiert Präsenz und Wertgleichheit
  beider Felder; bestehende Clients (ansV, syllogismus-fedlex) bleiben unberührt.

- **MCP-Protokoll-Default auf `2025-11-25` angehoben (live seit 2026-06-20).**
  Der `initialize`-Handshake handelt jetzt die höchste stabile Spec-Revision
  `2025-11-25` aus; `2024-11-05` bleibt für explizit nachfragende Alt-Clients
  (Negotiation) erhalten. Umgesetzt als Anhebung der Kompilier-Default
  `DEFAULT_PROTOCOL_VERSION` (nicht als reiner Env-Flip — Abweichung vom
  ursprünglichen Runbook-Plan 6.2, bewusst, weil der Server ohnehin neu gebaut
  und per Digest gepinnt wird). Rollback bleibt ein reiner Config-Flip:
  `MCP_PROTOCOL_DEFAULT=2024-11-05` (Runbook 2.3) **oder** Image-Digest zurück
  auf den Vorgänger (k3-infra `reader.yaml`). Live verifiziert (port-forward,
  Navigator-JWT): Default & zu neue Client-Version → `2025-11-25`, Client mit
  `2024-11-05` → exakt `2024-11-05`. Spec-Grundlage: ADR-008 §A, Runbook
  `docs/55_MIGRATION_mcp_protocol_upgrade.md`.

- **Input-Validation `2025-11-25`-konform**: beide `tools/call`-Pfade (fehlender
  `name`, ungültiges `as_of`) liefern jetzt einen in-band Tool-Execution-Error
  (`{ error, hint }` im `result`) statt eines `-32602`-Protokollfehlers; die
  Konstante `INVALID_PARAMS` wurde aus `transport.rs` entfernt. Eigene Tests
  sichern beide Pfade.

- **Lifecycle vervollständigt**: `id` ist nun `Option<Option<Value>>` mit
  `is_notification()`; der `rpc_handler` quittiert Notifications mit **HTTP 202
  ohne Body**, `ping` und `notifications/initialized` sind eigene Match-Arme.

## [0.1.0] - 2026-06-20



Erste getaggte Version. Der `mcp-reader` ist produktiv ausgerollt und gegen
Fedlex live-konform getestet.

### Added

- **MCP-Reader-Server** (Protokoll `2024-11-05`): `initialize`, `tools/list`,
  `tools/call` über JSON-RPC (`POST /rpc`) und SSE (`GET /sse`).
- **22 Werkzeuge** in drei Pools, RBAC-gefiltert (Reader ⊆ Navigator ⊆ Validator):
  - AKN-Navigation: `read_article`, `read_element`, `get_structure`, `search_text`,
    `get_metadata`, `read_document`, `get_references`, `get_modifications`.
  - Discovery: `search_law`, `resolve_sr_number`, `find_related_topic`.
  - JOLux-Metadaten/-Beziehungen: `check_in_force`, `list_versions`,
    `resolve_consolidation_at`, `get_impacts`, `get_outgoing_impacts`,
    `get_article_history`, `get_citations`, `get_taxonomy`, `get_subdivisions`,
    `list_annexes`.
  - Validierung: `compare_versions`.
- **Provenance-Gate** (ADR-004): jede Antwort trägt `eli` + `valid_as_of`; der
  Stichtag wird serverseitig gestempelt und ist nicht über Tool-Argumente
  verfälschbar.
- **Auth fail-closed** (ADR-002): JWT (HS256/RS256/JWKS mit Rotation) sowie
  `MCP_DEV_TOKEN` für die Entwicklung; Identität nie aus LLM-Parametern.
- **Verteilte Quota** (ADR-002): Token-Bucket in Redis, pool-gewichtet, fail-closed
  bei Redis-Ausfall.
- **Service-zu-Service-mTLS** Reader ↔ Quota-Redis (ADR-005), hinter Feature
  `redis-tls`; Klartext bei vorhandenem TLS-Material wird hart abgelehnt.
- **PII-Scrubbing & Tenant-Isolation** im Audit-Log (ADR-001, Allowlist).
- **Vollständigkeits-Matrix** der Lexikon-Projektion als Offline-Test
  (`tests/lexicon_projection.rs`, G-4-Schutz).
- **Datenschicht**: jolux- (29) + akn- (12) + bridge- (3) Live-Konformanztests,
  wöchentlich in CI.
- **Onboarding/Betrieb**: `docker-compose.yml`, `.env.example`, Quickstart-README,
  `docs/70_CONFIG.md`, `scripts/smoke.sh`, `docs/80_DEPLOY.md` (k8s + Runbook),
  `docs/90_AUTH_AND_ROLES.md`, `CONTRIBUTING.md`, `SECURITY.md`.
- **Distroless-Image** (nonroot, UID 65532), per Kaniko gebaut.

[Unreleased]: https://git.mindful-server.com/mindful-bio/mcp-fedlex/-/compare/v0.1.0...main
[0.1.0]: https://git.mindful-server.com/mindful-bio/mcp-fedlex/-/tags/v0.1.0
