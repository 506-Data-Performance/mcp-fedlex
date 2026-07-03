# 65 — Review-Findings (lebendes Register)

> **Was dieses Dokument ist.** Ein **laufend gepflegtes** Register für Befunde, die bei
> Code-/Doku-Reviews oder im Betrieb auffallen und **berücksichtigt oder korrigiert** werden
> müssen — bevor sie zu echten Lücken werden. Abgrenzung:
>
> - **[40_FINDINGS.md](40_FINDINGS.md)** ist ein *abgeschlossenes, themenspezifisches*
>   Befund-Protokoll (die Discovery-Lücke F-1, inzwischen gelöst).
> - **[60_OPEN_ITEMS_AND_USABILITY.md](60_OPEN_ITEMS_AND_USABILITY.md)** ist der *kuratierte,
>   am Code verifizierte* Fahrplan mit Abnahmekriterien.
> - **Dieses Dokument (65)** ist der *Eingangskorb* dazwischen: hier landet ein Finding zuerst,
>   wird bewertet, und wandert dann entweder in 60 (wird eingeplant), in einen ADR
>   (Entscheidung nötig) oder direkt nach „erledigt".
>
> **Status-Werte:** 🔴 offen · 🟡 in Arbeit · 🟢 erledigt · ⚪ verworfen (mit Begründung).

---

## Wie ein Finding einzutragen ist

Ein Eintrag ist erst vollständig, wenn er **Beleg** (Datei/Zeile oder Befehl), **Wirkung** und
**vorgeschlagene Behandlung** nennt. Format:

```
### RF-<n> — <Kurztitel>
- **Status:** 🔴/🟡/🟢/⚪
- **Entdeckt:** <Datum>, bei <Anlass>
- **Beleg:** <Datei:Zeile | Befehl | Test>
- **Wirkung:** <warum es zählt>
- **Behandlung:** <Korrektur | Verweis auf 60/ADR | bewusst verworfen>
```

---

## Register

### RF-1 — README war nicht mehr code-deckungsgleich (Tool-Zahl, Pools, Endpoints)
- **Status:** 🟢 erledigt (2026-06-21)
- **Entdeckt:** 2026-06-21, beim Abgleich README ↔ `crates/mcp-reader/src` (Frage „offene Punkte?").
- **Beleg:** `grep` der `name()`/`pool()`-Paare über `tools.rs`/`discovery.rs`/`metadata.rs` ⇒
  **25** registrierte Tools über **vier** aktive Pools (LocalNavigation 11, JoluxMetadata 10,
  Discovery 3, Validation 1); `protocol.rs:31/39` ⇒ `DEFAULT_PROTOCOL_VERSION = "2025-11-25"`,
  `SUPPORTED = ["2024-11-05","2025-11-25"]`; `transport.rs::mcp_handler` ⇒ Route `POST /mcp`.
- **Wirkung:** Der README nannte „22 Werkzeuge in drei Pools", listete drei real registrierte
  Tools nicht (`extract_tables`, `list_components`, `detect_foreign_content`) und kannte weder den
  `/mcp`-Streamable-HTTP-Endpoint noch `ping`/`notifications/initialized`. Für Fremdnutzer (das
  erklärte Ziel von Block A/C in [60](60_OPEN_ITEMS_AND_USABILITY.md)) eine falsche Provenance der
  eigenen Doku.
- **Behandlung:** README korrigiert — Tool-Zahl/Pools/Tool-Liste, Norm-vs-Hint-Provenance
  (`kind: "norm"|"hint"`, [ADR-006](adr/ADR-006-discovery-tools-and-hint-provenance.md)),
  Audit-Log-Highlight, drei HTTP-Routen (`/mcp`, `/rpc`, `/sse`) und der Methoden-Footprint
  inkl. `ping`/Notification ergänzt.

### RF-2 — Letzter offener Roadmap-Punkt: `v0.2.0` taggen (E-3)
- **Status:** 🟢 erledigt (Tag existiert seit 2026-06-21; Register am 2026-07-02 nachgeführt)
- **Entdeckt:** 2026-06-21, aus [60 §Block E / E-3](60_OPEN_ITEMS_AND_USABILITY.md).
- **Beleg:** 60 §E-3: „Verbleibend rein redaktionell: README-Badge auf `2025-11-25` heben und
  `v0.2.0` taggen." Konformanz-/Konsistenztests grün (160 Lib-Tests inkl. 6 `/mcp`-HTTP-Tests).
- **Wirkung:** Ohne Git-Tag `v0.2.0` baut die CI kein zitierbares SemVer-Image für Fremdnutzer
  ([60 §C-6](60_OPEN_ITEMS_AND_USABILITY.md)); README-Badge (`v0.2.0` / MCP `2025-11-25`) und
  tatsächlicher Release-Stand divergieren bis zum Tag.
- **Behandlung:** Erledigt — Tag `v0.2.0` zeigt auf `cbd919b` (`release: mcp-fedlex v0.2.0`,
  2026-06-21). Aufgefallen beim Web-Review ([66 §E-2](66_WEB_REVIEW_FINDINGS.md)): Das Register
  hinkte dem Tag knapp zwei Wochen hinterher, weil dieses Dokument selbst untracked war.

### RF-3 — Live-Pfad ungehärtet; Schutzschicht vorgebaut, aber unverdrahtet
- **Status:** 🟢 erledigt (2026-07-02) — Blöcke H/O/W/T aus [67](67_HARDENING_AND_SOTA_ROADMAP.md) umgesetzt (`17a93f2…9537cc7`); dort offen nur P-1 (wartet auf Freigabe), P-3…P-5
- **Entdeckt:** 2026-07-02, bei zwei Code-Audits (Stabilität; Primitive/Spec) nach Abschluss
  der 50/60-Fahrpläne.
- **Beleg:** `reqwest::Client` ohne jedes Timeout (`sparql_http.rs:30`, `xml_source.rs:23`);
  Router ohne Timeout-/Concurrency-Layer (`transport.rs:637-643`); `CircuitBreaker`,
  Single-Flight-`L1Cache`, Timeout-`Sandbox`, `WarmupCache`, `SemanticClient` exportiert und
  getestet, aber 0 Referenzen aus `main/transport/registry/tools/discovery/metadata`;
  Redis-Connection pro Aufruf ohne Op-Timeout (`token_bucket.rs:118`); kein Graceful
  Shutdown (`app.rs:42`); Logging = `println!` (`transport.rs:464`), kein `/metrics`;
  `/readyz` hängt an Live-Fedlex (`probes.rs:78`).
- **Wirkung:** Wird Fedlex *langsam* (nicht down), staut sich der Server ungebremst —
  Task-/Socket-Erschöpfung bis zum Pod-Kipp; Rolling-Deploys brechen In-Flight-Requests;
  im Vorfall fehlt jede Metrik-Sicht. Die „State of the Art"-Bausteine existieren bereits —
  sie sind nur nie an den Live-Pfad angeschlossen worden.
- **Behandlung:** Priorisierter Fahrplan mit Abnahmen in
  [67_HARDENING_AND_SOTA_ROADMAP.md](67_HARDENING_AND_SOTA_ROADMAP.md) (Blöcke H/O/W;
  P0 = H-1 Timeouts, H-2 Router-Limits, H-3 Breaker verdrahten). Dort auch die
  Protokoll-Punkte (P-1 `content[]`/`structuredContent` als größte Wire-Abweichung).

### RF-4 — Doku-Drift: ADR-008 „Proposed", Runbook & Briefing hinken dem Code hinterher
- **Status:** 🟢 erledigt (2026-07-02, `9537cc7` + `859b9b7` — inkl. dritter Namensdrift, die die neue Wache selbst fand)
- **Entdeckt:** 2026-07-02, bei denselben Audits.
- **Beleg:** [ADR-008](adr/ADR-008-mcp-protocol-version-upgrade.md) Status „Proposed", obwohl
  umgesetzt und `v0.2.0` getaggt; [55_MIGRATION](55_MIGRATION_mcp_protocol_upgrade.md) mit
  offenen Checkboxen (3.2, 4.2, 7.x, 8.3-8.5) trotz erledigter Arbeit;
  [IMPLEMENTATION_BRIEFING…](IMPLEMENTATION_BRIEFING_lexicon_projection_matrix.md) nennt
  21/26/22 statt 24/23/25; [50_ROADMAP](50_ROADMAP_TO_PERFECT.md) nennt 22 Tools und markiert
  die eigene Erledigung nicht. Zudem zwei Funktionsnamen-Drifts Doku↔Code
  (`resolve_vocabulary_term/label`, `parse_unlinked_refs/ref`).
- **Wirkung:** Genau die Drift-Sorte, die RF-1 an der README korrigiert hat — nur eine Ebene
  tiefer; wer den Docs folgt, implementiert gegen einen alten Stand.
- **Behandlung:** [67 §T-3](67_HARDENING_AND_SOTA_ROADMAP.md) (Sammel-Nachzug) und
  [67 §T-2](67_HARDENING_AND_SOTA_ROADMAP.md) (Lexikon-Wache code-getrieben + Namensabgleich).

### RF-5 — Redis-Passwort im Klartext im Startup-Log (Credential-Leak in Logs)
- **Status:** 🔴 offen
- **Entdeckt:** 2026-07-03, beim Prod-Smoke der Agent-UX-Welle (`kubectl logs` der frisch
  ausgerollten Reader-Pods, Digest `1703721…`).
- **Beleg:** [`crates/mcp-reader/src/main.rs:250`](../crates/mcp-reader/src/main.rs#L250) und
  [`:255`](../crates/mcp-reader/src/main.rs#L255) loggen das strukturierte Feld `redis_url` auf
  INFO. `REDIS_URL` kommt in Prod aus dem SealedSecret `mcp-reader-redis-auth` in der Form
  `rediss://default:<passwort>@mcp-reader-redis:6379` — das **Passwort steht damit im Klartext**
  in stdout. Verifiziert an der laufenden Instanz: `mcp_reader: Quota-Redis über mTLS verbunden
  (ADR-005) redis_url="rediss://default:<redigiert>@mcp-reader-redis:6379"`.
- **Wirkung:** Jedes Log-Ziel, das stdout einsammelt, erhält das Redis-Passwort — im Cluster
  **Promtail → Loki** (`kube-prometheus-stack`, k3-infra), potenziell also ein zentraler,
  breiter zugänglicher Log-Store und in Backups. Bricht die Grundregel der eigenen
  Compliance-Schicht (`fedlex-telemetry`: „nichts Sensibles verlässt den Prozess unmaskiert");
  der vorhandene allowlist-Scrubber greift für Span-Attribute, **nicht** für dieses direkte
  `tracing::info!`. Entschärfend, aber nicht heilend: Default-Deny-NetworkPolicy, das Passwort
  rotiert mit dem SealedSecret, und es ist ein Infra- (kein Mandanten-)Credential — daher
  Härtung/Defense-in-Depth, kein akuter Incident.
- **Behandlung:** URL vor dem Logging redigieren — nur `scheme://host:port` ausgeben, `userinfo`
  (alles zwischen `//` und `@`) durch `***` ersetzen; am saubersten als kleiner
  `RedactedRedisUrl`-Newtype mit eigener `Display`-Impl, sodass die rohe URL nie versehentlich
  in ein `tracing`-Feld geraten kann (Typsystem statt Disziplin — analog `Sensitive<T>`).
  Beide Logzeilen (mTLS + Klartext-Fallback) umstellen; Regressionstest, der die gerenderte
  Logzeile auf Abwesenheit des `:<pw>@`-Musters prüft. Klein und risikoarm, zieht aber einen
  Rebuild + Redeploy nach sich → als Härtungs-Punkt einplanbar
  ([67](67_HARDENING_AND_SOTA_ROADMAP.md)). **Sicherheitsfund** — gemäss
  [CLAUDE.md](../CLAUDE.md) nicht als öffentliches Issue führen (GitHub ist öffentlicher
  Mirror); Behandlung hier + ggf. `SECURITY.md`, nicht im Bugtracker.

---

## Triage-Regel (ein Satz)

Ein neues Finding kommt zuerst hierher (🔴), wird bewertet und wandert dann an seinen
Ziel-Ort: einplanbar ⇒ nach [60](60_OPEN_ITEMS_AND_USABILITY.md), entscheidungsbedürftig ⇒ ein
neuer ADR unter [adr/](adr/), sofort behebbar ⇒ Korrektur + Status 🟢 mit Beleg.
