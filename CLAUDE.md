# mcp-fedlex

Model-Context-Protocol-Server für **Fedlex** (Schweizer Bundesrecht), Produkt von
mindful.bio. Gibt einem LLM **zitierfähigen**, zeitpunktgenauen Zugriff auf
konsolidiertes Bundesrecht. Rust-Workspace, Edition 2024, MCP-Protokoll `2025-11-25`
(Legacy `2024-11-05` nur auf explizite Anfrage). RBAC: Reader ⊆ Navigator ⊆ Validator;
40 Tools über vier Pools (LocalNavigation / Discovery / JoluxMetadata / Validation; ADR-010).

> README.md und CONTRIBUTING.md sind die ausführliche **Quelle der Wahrheit**. Diese
> Datei ist die destillierte, immer geladene Kurzfassung — bei Konflikt gewinnen jene.

## Befehle (CI-Gleichlauf)

```bash
# Vor jedem Commit — exakt das prüft die Pipeline:
cargo fmt --all                          # formatieren
cargo fmt --all --check                  # muss sauber sein
cargo clippy --workspace -- -D warnings  # Lints sind hart
cargo test --workspace                   # Unit/Integration, KEIN Netzwerk

cargo build --workspace
cargo test --workspace -- --ignored      # Live-Konformanz: Docker/Redis + echtes Fedlex

# Lokaler Lauf (kein Rust nötig, nur Docker):
cp .env.example .env
docker compose up --build                # Reader auf http://localhost:8080
scripts/smoke.sh http://localhost:8080 dev-secret-change-me
MCP_HOST_PORT=8090 docker compose up --build   # Host-Port umbiegen (Container bleibt 8080)

# Tools interaktiv durchklicken:
npx -y @modelcontextprotocol/inspector --config inspector.json --server fedlex
```

## Crates (`crates/`)

- **fedlex-core** — geteilte Kern-Typen; kodiert die schicht­übergreifenden Invarianten.
- **fedlex-store** — Data-Access-Layer, Tenant-Isolation (ADR-001).
- **fedlex-jolux** — getestete, komponierbare JOLux-SPARQL-Primitive (Metadaten/Graph); geht **live** ans öffentliche Fedlex-Endpoint.
- **fedlex-akn** — AKN-4.0-Lexikon als Funktionen (20 Primitive, `docs/11_LEXICON_akn.md`); lokale Navigation im Akt-Volltext.
- **fedlex-bridge** — produktiver Pfad JOLux (Metadaten) → AKN (Volltext); bewusst **transportfrei**.
- **fedlex-telemetry** — PII-Scrubber + `Sensitive`-Typen (Compliance-Gate); das Laufzeit-Logging (`tracing`, `RUST_LOG`/`MCP_LOG_FORMAT`) lebt im mcp-reader.
- **mcp-reader** — das Binary: zustandsloser MCP-Reader (CQRS-Leseseite); Auth/RBAC, verteilte Quota, HTTP-Routen `/mcp` · `/rpc` · `/sse`.

## Architektur-Invarianten (ADR-gestützt, test-abgesichert — NICHT verletzen)

- **Identität nie aus LLM-Parametern** (ADR-002): `tenant`/`session`/`role` nur aus geprüften Claims, nie aus einem Tool-Argument.
- **Provenance an jeder Normantwort** (ADR-004): trägt `eli` + `valid_as_of`; das Datum wird **serverseitig** gestempelt und ist nicht manipulierbar.
- **norm vs. hint**: Normzitat (`kind: "norm"`) und Discovery-Hinweis (`kind: "hint"`, Kandidat) strukturell getrennt — ein Hint darf nie als Zitat verbucht werden.
- **PII-Scrubbing im Audit-Log** (ADR-001): keine rohen Tool-Argumente/Antwortinhalte loggen.
- **Least-Privilege-Pools** (ADR-006/007): jedes neue Tool braucht einen `ToolPool` **und** einen Eintrag in der Projektions-Matrix (`crates/mcp-reader/tests/lexicon_projection.rs`), sonst wird der Test rot.
- **fail-closed**: Auth- und Quota-Pfade verweigern im Zweifel.
- Domänen-Crates bleiben **transportfrei** (client/WASM- wie serverseitig nutzbar).

## Wo was liegt

- Code: `crates/` · Architektur-Plan: `likec4/`
- Lexika & Pläne: `docs/` (`10_LEXICON_jolux`, `11_LEXICON_akn`, `30_PLAN`, `45_GAP_ANALYSIS`, `50_ROADMAP_TO_PERFECT`)
- Entscheidungen: `docs/adr/` (ADR-001 … ADR-010)
- Betrieb/Config: `docs/70_CONFIG.md` (alle Env-Vars), `docs/80_DEPLOY.md`, `docs/90_AUTH_AND_ROLES.md`
- Lebende Review-Register: `docs/65_REVIEW_FINDINGS.md` (Code), `docs/66_WEB_REVIEW_FINDINGS.md` (Website), `docs/68_AGENT_UX_FINDINGS.md` (Agent-UX/Dogfooding)
- Dev-Anleitung (maßgeblich): `CONTRIBUTING.md`

## Konventionen

- **Conventional Commits**: `feat(scope):`, `fix(scope):`, `docs:`, `style:`. Das **Warum** beschreiben, auf ADRs/Gaps verweisen. Kleine, fokussierte Commits; CI muss grün sein.
- GitHub ist ein **öffentlicher Mirror**; Quelle der Wahrheit (CI/CD, Releases) ist das selbst-gehostete GitLab. Release = Git-Tag `vX.Y.Z`; `serverInfo.version` == Version in `Cargo.toml`.
- Sicherheitsfunde **nie** als öffentliches Issue → `SECURITY.md`.

## Diese Datei pflegen

Aktualisiere die CLAUDE.md **in derselben Änderung**, wenn sich Build-/Test-Befehle, die
Crate-Struktur oder eine ADR-Invariante ändern. Hier gehört nur **Stabiles** hin —
Volatiles (Status, Roadmap, Findings) bleibt in `docs/` und wird von hier nur verlinkt.
So bleibt das Dokument klein und veraltet kaum.
