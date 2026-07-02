# 67 — Härtung & Protokoll-SOTA (Fahrplan nach Abschluss von 50/60)

> **Was dieses Dokument ist.** Der Nachfolger von [50_ROADMAP_TO_PERFECT.md](50_ROADMAP_TO_PERFECT.md)
> und [60_OPEN_ITEMS_AND_USABILITY.md](60_OPEN_ITEMS_AND_USABILITY.md) — deren Blöcke sind
> vollständig abgearbeitet (25 Tools testverankert, MCP `2025-11-25` live, `v0.2.0` getaggt).
> Dieses Dokument bündelt die **nächste Welle**: Produktions-Stabilität und die verbleibende
> Distanz zu „State of the Art". Grundlage sind zwei Code-Audits vom **2026-07-02**
> (Stabilität/Robustheit; Primitive/MCP-Spec) plus grüner CI-Gleichlauf (fmt, clippy,
> alle Tests). Jeder Punkt trägt Beleg (Datei:Zeile) und Abnahme.
> **Status-Werte:** 🔴 offen · 🟡 in Arbeit · 🟢 erledigt · ⚪ verworfen.
>
> **Abarbeitung 2026-07-02:** Blöcke **H, O, W, T vollständig umgesetzt** plus P-2
> (Commits `17a93f2…9537cc7`, jeder mit Test-Abnahme; CI-Gleichlauf durchgehend grün).
> Offen: **P-1** (content[]/structuredContent — wartet auf Freigabe, Breaking Change
> inkl. Konsumenten-Nachzug), **P-3** (OAuth-Discovery-Fassade), **P-4** (`resources`,
> Produktentscheidung), **P-5** (Result-Pagination).

---

## 0. Gesamtbild

Die Substanz ist exzellent: **null `unwrap()`/`panic!` im Produktionscode**, ~370 Tests,
fail-closed Auth, saubere Config-Härte, Protokoll-Kern bereits auf `2025-11-25`
(Streamable HTTP `/mcp` mit Origin/Version-Wächtern). Der produktive Tool-Pfad nutzt die
Lexikon-Primitive sauber — die Tools sind dünne Hüllen, die Bridge komponiert über
`resolve_consolidation_at`, kein SPARQL-Bypass.

**Der Kernbefund liegt eine Ebene tiefer: zwei parallele Welten.** `main.rs` verdrahtet
die *naive* Variante des Live-Pfads, während die „State of the Art"-Bausteine fertig
gebaut, exportiert und getestet daneben liegen — von keiner produktiv komponierenden
Datei referenziert (grep über `main/transport/registry/tools/discovery/metadata` = 0 Treffer):

| Vorgebaut & tot | Datei | Produktiver Ersatz heute |
|---|---|---|
| `CircuitBreaker` + `LodGateway` | `mcp-reader/src/circuit_breaker.rs`, `lod_gateway.rs` | Discovery/Metadata/Fetcher gehen **roh** gegen Fedlex |
| Single-Flight-`L1Cache` (`get_with`) | `mcp-reader/src/xml_engine.rs:88` | moka-Cache ohne Stampede-Schutz (`fetcher.rs:68-72`) |
| Timeout-`Sandbox` (`spawn_blocking` + Deadline) | `mcp-reader/src/sandbox.rs:83` | XML-Parse läuft **inline** auf dem Runtime-Thread (`fetcher.rs:71`) |
| `WarmupCache` | `mcp-reader/src/warmup.rs:61` | `health.mark_started()` sofort (`main.rs:92`) |
| `SemanticClient` | `mcp-reader/src/semantic_client.rs` | — (semantic-fedlex noch nicht angebunden) |
| `ToolPool::LodFederation`, `ToolPool::Workspace` | `tool.rs:23,44` | deklariert, unbestückt (Fussnote in [90](90_AUTH_AND_ROLES.md) §3) |

Dazu **Duplikate**: `compare_versions` rechnet den Fassungs-Diff von Hand nach
(`tools.rs:604-703`), obwohl `xml_engine::diff_to_markdown` (`xml_engine.rs:105-142`)
dieselbe getestete Logik liefert; `paginate()`/`Page` (`xml_engine.rs:158-173`) sind
getestet und ungenutzt; `xml_engine::Document::parse` ist ein Stub-Schatten des echten
AKN-Parsers.

**Konsequenz:** Wird Fedlex langsam (nicht down — *langsam*), staut sich der Server
ungebremst: keine HTTP-Timeouts, kein Request-Timeout, kein Concurrency-Limit, kein
Breaker. Genau das ist heute das größte Stabilitätsrisiko.

### Verifiziert stark (nicht anfassen)

| Geprüft | Beleg |
|---|---|
| Keine Panik-Pfade: 0 `unwrap()`, `expect()` nur als Lock-Poisoning-Guards / beweisbar sicher | Audit §1; z. B. `genesis.rs:56` |
| Tool-Pfad nutzt Primitive 1:1, kein SPARQL/XML-Bypass im produktiven Pfad | `tools.rs:191`, `metadata.rs:217`, `fetcher.rs:14,60` |
| Lexikon-Vollständigkeit (47 = 24 + 23) testverankert, Zähl-Invarianten grün | `lexicon_projection.rs:362-375` |
| Config-Härte: sauberer Startabbruch statt Panik, fail-safe Defaults | `main.rs:41,130,150-173` |
| Quota fail-closed inkl. Fallback-Bucket, atomares Lua-Token-Bucket | `quota.rs:119-125,258`, `token_bucket.rs:43` |

---

## Block H — Härtung des Live-Pfads (P0/P1, höchster Hebel)

### H-1 — Timeouts auf allen reqwest-Clients 🟢 (2026-07-02, `17a93f2`)
- **Beleg:** `sparql_http.rs:30`, `xml_source.rs:23`, `main.rs:205` — `reqwest::Client::new()`/
  `default()` ohne `connect_timeout`/`timeout`; `send().await` kann Minuten hängen.
- **Abnahme:** Jeder Client hat `connect_timeout` (~3 s) + `timeout` (~15 s, konfigurierbar
  via Env, dokumentiert in [70_CONFIG](70_CONFIG.md)); ein Test mit verzögertem Mock-Server
  beweist den Abbruch.

### H-2 — Request-Timeout + Concurrency-Limit am Router 🟢 (2026-07-02, `2cb9a95`)
- **Beleg:** `transport.rs:637-643` — Router ohne jedes `.layer(...)`; unbegrenzte
  gleichzeitige Requests × hängende Fetches = Task-/Socket-Erschöpfung.
- **Abnahme:** tower-Layer (`TimeoutLayer`, `ConcurrencyLimit`/`LoadShed`, Limits via Env);
  Überlast liefert eine lenkende Fehlerantwort statt Stau; Test simuliert N parallele
  langsame Aufrufe.

### H-3 — CircuitBreaker in Discovery/Metadata/Fetcher verdrahten 🟢 (2026-07-02, `c7fdef8`)
- **Beleg:** `circuit_breaker.rs` getestet, aber nur vom toten `lod_gateway.rs:107` genutzt;
  der produktive `AknFetcher` ruft `self.sparql`/`self.source` direkt.
- **Abnahme:** Live-SPARQL/XML-Pfade laufen durch den Breaker; offener Breaker ⇒ sofortige
  lenkende Antwort (`{error, hint, retry_after_ms}`); Zustands-Test (closed→open→half-open).

### H-4 — Redis: Op-Timeout + Connection-Reuse 🟢 (2026-07-02, `7d27009`)
- **Beleg:** `token_bucket.rs:118/127`, `redis_store.rs:40-54` — neue Multiplexed-Connection
  **pro Aufruf** (mit mTLS: TLS-Handshake pro Request!), keine Zeitgrenze; fail-closed greift
  nur bei schnellem `Err`, nicht bei *hängendem* Redis.
- **Abnahme:** geteilte Connection (Reuse) + `tokio::time::timeout` um Redis-Ops; Test
  „Redis hängt" fällt in den Fallback-Bucket statt zu blockieren.

### H-5 — Single-Flight + gewichtsbasierte Eviction im Manifestations-Cache 🟢 (2026-07-02, `a341256`)
- **Beleg:** `fetcher.rs:38` (`Cache::new(64)`, zählbasiert, „1–10 MB pro Erlass" ⇒ bis
  ~640 MB), `fetcher.rs:68-72` (bewusst kein Single-Flight ⇒ N parallele Misses = N
  Downloads + N Parses). Die Lösung liegt ungenutzt in `xml_engine.rs:88` (`get_with`).
- **Abnahme:** moka mit `weigher` (XML-Größe) + `get_with`-Single-Flight **im Fetcher**
  (Logik dorthin umziehen, nicht das Stub-Modul verdrahten); Stampede-Test: N parallele
  Misses ⇒ genau 1 Download/Parse.

### H-6 — XML-Parse via `spawn_blocking` + Download-Größenlimit 🟢 (2026-07-02, `78a9955`)
- **Beleg:** `dom.rs:86` (CPU-gebundener Parse) inline in `fetcher.rs:71`; `xml_source.rs:48`
  lädt Bodies ohne Cap in einen `String`.
- **Abnahme:** Parse in `spawn_blocking` mit Deadline (Muster aus `sandbox.rs:83`
  übernehmen); Download bricht über konfigurierbarem Limit ab; Tests für beide Grenzen.

### H-7 — Graceful Shutdown 🟢 (2026-07-02, `4ab557d`)
- **Beleg:** `app.rs:42` — `axum::serve` ohne `.with_graceful_shutdown()`; kein
  SIGTERM-Handler ⇒ K8s-Rolling-Deploy bricht In-Flight-Requests hart ab (502).
- **Abnahme:** SIGTERM/ctrl_c-Handler, Drain-Fenster; Test oder dokumentierter manueller
  Nachweis (Deploy ohne 502 im Smoke).

### H-8 — Readiness von Fedlex entkoppeln 🟢 (2026-07-02, `11ccf26`)
- **Beleg:** `probes.rs:78-80` — `/readyz` prüft live Fedlex-SPARQL; Fedlex-Ausfall nimmt
  **alle** Pods aus dem LB, obwohl Cache/lokale Navigation funktionieren würden.
- **Abnahme:** `/readyz` prüft nur eigene Abhängigkeiten (Redis) bzw. Fedlex mit kurzem
  Timeout als *degraded*-Signal statt Unready; Verhalten in [80_DEPLOY](80_DEPLOY.md)
  dokumentiert.

---

## Block O — Observability (heute: 14 × `println!`)

### O-1 — Strukturiertes Logging via `tracing` 🟢 (2026-07-02, `cae3f25`)
- **Beleg:** kein `tracing`/`log` im Workspace (grep Cargo.toml/src leer); Audit-Zeile via
  `println!` (`transport.rs:464`) nimmt den globalen stdout-Lock auf dem Request-Pfad.
- **Abnahme:** `tracing` + `tracing-subscriber` (JSON), Audit-Event als strukturiertes
  Event hinter dem bestehenden PII-Scrubber; Log-Level via Env.

### O-2 — `/metrics` (Prometheus) 🟢 (2026-07-02, `e9557c0`)
- **Beleg:** kein Metrics-Endpoint; keine Sicht auf Fehlerrate, Latenz, Cache-Hit-Quote,
  `degraded`-Quote (Redis-Fallback), Breaker-Zustand.
- **Abnahme:** `/metrics` mit Request-/Fehler-/Latenz-Histogrammen je Tool-Pool,
  Upstream-Latenz, Cache-Hits, Quota-Fallback-Zähler, Breaker-Zustand; am Ingress **nicht**
  öffentlich geroutet (nur in-cluster).

### O-3 — CLAUDE.md-Korrektur «Tracing-Layer» 🟢 (2026-07-02, `cae3f25`)
- **Beleg:** CLAUDE.md nennt `fedlex-telemetry` „Tracing-Layer + PII-Scrubber" — real ist es
  nur der Scrubber (`telemetry/src/lib.rs`).
- **Abnahme:** Formulierung korrigiert (bzw. nach O-1 wieder wahr).

---

## Block W — Verdrahtungsruinen aufräumen (eine Wahrheit pro Funktion)

> Grundsatz: **Kein exportiertes Modul ohne produktiven Nutzer.** Entweder verdrahten
> (wo H-Punkte es ohnehin brauchen) oder löschen — Git vergisst nichts.

### W-1 — Entscheidung je totem Modul 🟢 (2026-07-02, `8e433e3` — Module gelöscht, Ideen leben in H-3/H-5/H-6; Pools bleiben reserviert)
- `sandbox.rs` → Muster geht in **H-6** auf (danach Modul löschen oder als echten Wrapper nutzen).
- `xml_engine.rs` → Single-Flight-Idee geht in **H-5** auf; `Document::parse`-**Stub** und
  `L1Cache` danach **löschen** (Schatten-Parser ist ein Irrtums-Risiko); `paginate`/`Page`
  nach **P-5** umziehen oder mitlöschen; `diff_to_markdown` s. W-2.
- `lod_gateway.rs`/`ToolPool::LodFederation` → behalten **nur mit** datiertem Reaktivierungs-
  Vermerk (LOD-Milestone), sonst löschen.
- `semantic_client.rs`/`ToolPool::Workspace` → dito (semantic-fedlex-Anbindung).
- `warmup.rs` → entweder echten Warmup in `main.rs:90-92` schalten (BV/OR/ZGB vorladen)
  oder löschen.
- **Abnahme:** grep „exportiert-aber-unreferenziert" ist leer bzw. jeder Rest trägt einen
  begründeten Reservierungs-Kommentar mit Datum (analog Projektions-Matrix).

### W-2 — `compare_versions` auf eine Diff-Implementierung reduzieren 🟢 (2026-07-02, `8e433e3` — Stub-Duplikat mit xml_engine entfernt)
- **Beleg:** Diff-Logik doppelt: `tools.rs:654-701` (produktiv, handgerechnet) vs.
  `xml_engine.rs:105-142` (getestet, tot).
- **Abnahme:** genau eine Implementierung, von beiden Tests abgedeckt; Wire-Format unverändert
  (Baseline-Test bleibt grün).

---

## Block P — Protokoll-SOTA (MCP `2025-11-25`, additiv)

### P-1 — `content[]`-Envelope + `structuredContent`/`outputSchema` 🔴 **P1** — **wartet auf Freigabe** (Breaking Change, zieht ansV & syllogismus-fedlex nach; ADR nötig)
- **Beleg:** `tools/call` liefert das Domänen-Objekt `{data, provenance}` **roh** als
  JSON-RPC-`result` (`transport.rs:461-473`, `registry.rs:97-104`) — weder `content[]` noch
  `structuredContent`; testverriegelt in `protocol_baseline.rs:270-296`. Generische
  MCP-Clients (Claude Desktop, Inspector) erwarten die Spec-Hülle.
- **Wirkung:** größte verbleibende Wire-Abweichung; blockiert generische Client-Nutzung.
- **Abnahme:** `CallToolResult` mit `content[]` + `structuredContent` (der `{data,provenance}`-
  Block ist bereits strukturiert) + `outputSchema` je Tool; Konsumenten (ansV,
  syllogismus-fedlex) im selben Schritt nachziehen (Muster ADR-008: keine Rückwärts-
  Kompatibilität als Ziel); Baseline-Tests auf die neue Hülle umgestellt; MCP-Inspector
  zeigt strukturierte Antworten.

### P-2 — Tool-Annotations (`readOnlyHint` etc.) 🟢 (2026-07-02, `daef198`)
- **Beleg:** 0 Annotations im Code; alle 25 Tools sind read-only, Live-Tools open-world.
- **Abnahme:** `annotations` je Tool (`readOnlyHint: true`, `idempotentHint`,
  `openWorldHint` für Discovery/JoluxMetadata); im `tools/list`-Test verankert.

### P-3 — OAuth-RS-Fassade: 401/`WWW-Authenticate` + `.well-known` 🔴 **P2**
- **Beleg:** Auth-Fehler liefern HTTP 200 + `-32001`-Body auch auf `/mcp`
  (`transport.rs:329-341,616-617`); kein `.well-known/oauth-protected-resource` (RFC 9728).
  Substanz (JWT/JWKS fail-closed) ist da — nur die Discovery-Fassade fehlt.
- **Abnahme:** `/mcp` antwortet bei fehlender/ungültiger Auth mit **401 +
  `WWW-Authenticate`** (Legacy `/rpc` unverändert für Alt-Clients);
  `.well-known/oauth-protected-resource` ausgeliefert; Ingress-Route ergänzt (k3-infra).

### P-4 — `resources` + Resource-Links (ELI als adressierbare Ressource) 🔴 **P3** *(Produktentscheidung, ADR)*
- **Beleg:** capabilities test-verriegelt auf `{tools:{}}` (`protocol_baseline.rs:203-221`);
  für Gesetzestexte wäre `resources/read` (ELI-URI) + Resource-Links in Tool-Antworten der
  naheliegendste Produkt-Hebel — aber echte Neuentwicklung.
- **Abnahme:** ADR mit Scope-Entscheid; falls ja: `resources`-Capability, RBAC/Quota/
  Provenance-Gate identisch zu Tools.

### P-5 — Result-Pagination für große Antworten 🔴 **P3**
- **Beleg:** `read_document`/`search_text` liefern unbegrenzte Strukturen; `paginate()`
  liegt tot in `xml_engine.rs:158-173`.
- **Abnahme:** Cursor-Parameter für die großen Tools, Grenzen dokumentiert; verhindert
  Context-Overflow der Agenten.

---

## Block T — Tests & Doku-Nachzug

### T-1 — Fehlerpfad-Tests für die neue Härtung 🟢 (2026-07-02 — als Abnahme in jedem H-Commit: hängender Upstream, Stampede, hängendes Redis, Overload, Drain)
- **Beleg:** Mocks antworten instant; es gibt keine Timeout-/Stampede-/Hänge-Tests
  (Audit §10); `fedlex-bridge` (der Hot-Path!) hat nur 4 Unit-Tests.
- **Abnahme:** je H-Punkt ein Fehlerpfad-Test (langsamer Mock via `tokio::time`,
  Stampede-Zähler, hängendes Redis); Bridge-Deckung deutlich erhöht.

### T-2 — Lexikon-Wache code-getrieben ergänzen 🟢 (2026-07-02, `859b9b7` — fand selbst eine dritte Drift: `resolve_version_at`)
- **Beleg:** `lexicon_projection.rs` prüft Lexikon↔Matrix↔Registry, aber nicht Code→Lexikon;
  Funktionsnamen werden extrahiert und verworfen — zwei Namensdrifts unbemerkt:
  `resolve_vocabulary_term` (Doku) vs. `resolve_vocabulary_label` (`fedlex-jolux/src/lib.rs:63`),
  `parse_unlinked_refs` vs. `parse_unlinked_ref` (`fedlex-akn/src/lib.rs:38`).
- **Abnahme:** Test vergleicht exportierte Primitive gegen Lexikon-IDs **und**
  Funktionsnamen; die zwei Drifts sind behoben (Doku oder Code, eine Wahrheit).

### T-3 — Doku-Drift beheben 🟢 (2026-07-02, `9537cc7`)
- **Beleg:** [ADR-008](adr/ADR-008-mcp-protocol-version-upgrade.md) steht auf „Proposed",
  ist aber umgesetzt; Runbook [55](55_MIGRATION_mcp_protocol_upgrade.md) hat offene
  Checkboxen (3.2, 4.2, 7.x, 8.3-8.5) trotz erledigter Arbeit;
  [IMPLEMENTATION_BRIEFING…](IMPLEMENTATION_BRIEFING_lexicon_projection_matrix.md) nennt
  21/26/22 statt **24/23/25**; [50_ROADMAP](50_ROADMAP_TO_PERFECT.md) nennt 22 Tools und
  kennt seine eigene Erledigung nicht.
- **Abnahme:** ADR-008 → „Accepted"; Runbook-Checkboxen nachgeführt; Briefing auf 24/23/25;
  50 erhält Kopf-Vermerk „abgeschlossen — Nachfolger: dieses Dokument".

---

## Priorisierung (Reihenfolge in einem Satz)

Erst den Live-Pfad **überlebensfähig** machen (H-1…H-3 = P0: Timeouts, Limits, Breaker),
dann **Effizienz & Sichtbarkeit** (H-4…H-6, O-1/O-2), parallel die **Ruinen aufräumen** (W),
dann die **Wire-Form** auf Spec heben (P-1, P-2), zuletzt Kür (P-3…P-5) — und T-1/T-3
laufen als Abnahme-Disziplin in jedem Schritt mit.

## Nicht-Ziele (bewusst, unverändert)

- **Tranche D** (treaties/genesis/publication/vocabulary — 11 der 23 reservierten Primitive,
  alle implementiert und live-getestet) sowie `get_law_metadata`/`extract_change_notes`/
  `parse_unlinked_ref`: **weiter nur bei belegtem ansV-Bedarf** projizieren
  ([60 §4](60_OPEN_ITEMS_AND_USABILITY.md)). Reifste Kandidaten, falls Bedarf entsteht:
  `get_law_metadata` (RES-03), Publikations-Tranche (PUB).
- **elicitation / sampling / tasks / prompts / SSE-Streaming / Session-Management**:
  widersprechen dem zustandslosen Read-only-Modell — Ausschluss bleibt (ADR-008 §B-6).
- **Eigener IdP / Registry-Marketing** (`server.json`): erst nach P-3, rein Distribution.
