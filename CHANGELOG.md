# Changelog

Alle nennenswerten Änderungen an diesem Projekt werden hier festgehalten.

Das Format orientiert sich an [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
und das Projekt folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

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

- **Upstream-Timeouts (67 §H-1):** Alle ausgehenden HTTP-Clients (Fedlex-SPARQL,
  AKN-Filestore, JWKS-Abruf) tragen zwingend Connect- (3 s) und Gesamt-Timeout
  (15 s), konfigurierbar über `MCP_UPSTREAM_CONNECT_TIMEOUT_MS` /
  `MCP_UPSTREAM_TIMEOUT_MS` (70_CONFIG §1a). Neu `fedlex_bridge::HttpTimeouts`;
  `HttpSparqlClient::new/fedlex` und `HttpXmlSource::new` sind dadurch fallible
  (sauberer Startabbruch statt reqwest-Panik bei TLS-Init-Fehlern). Abnahme:
  `tests/http_timeouts.rs` beweist den Abbruch gegen einen hängenden Upstream.
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
