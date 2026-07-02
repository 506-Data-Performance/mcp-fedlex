# ADR-009: `tools/call`-Antworten in der Spec-Hülle (`content[]` + `structuredContent` + `outputSchema`)

- **Status:** Accepted — umgesetzt 2026-07-02 (67 §P-1)
- **Datum:** 2026-07-02
- **Kontext-Artefakt:** `crates/mcp-reader/src/registry.rs` (Verpackung), `transport.rs` (Audit/Metrik/Validierungsfehler)
- **Betrifft:** `mcp-fedlex` (Reader) und beide Rust-Konsumenten — ansV (`ansv-fedlex::McpClient`), syllogismus-fedlex (`McpFedlexClient`) — sowie alle generischen MCP-Clients (Claude Desktop, Inspector, SDKs)
- **Folge-Release:** Ziel `v0.3.0` (siehe `CHANGELOG.md`)

## Kontext

`tools/call` lieferte das Domänen-Objekt `{data, provenance}` (bzw. lenkend
`{error, hint}`) **roh** als JSON-RPC-`result` — eine hauseigene Wire-Form,
testverriegelt in `protocol_baseline.rs`. Die MCP-Spec (`2025-11-25`, wie alle
Revisionen seit `2024-11-05`) verlangt ein `CallToolResult`:

```json
{ "content": [ { "type": "text", "text": "…" } ],
  "structuredContent": { … },
  "isError": false }
```

**Wirkung der Abweichung:** Generische MCP-Clients parsen `result.content[]`
und finden nichts — der Server funktioniert nur mit Clients, die das Hausformat
kennen. Für einen öffentlich beworbenen Server ist das die größte verbleibende
Konformitätslücke (Audit 2026-07-02, [67 §P-1](../67_HARDENING_AND_SOTA_ROADMAP.md)).

## Entscheidung

1. **Eine Wire-Form überall** (Muster ADR-008: Rückwärtskompatibilität ist kein
   Ziel; Konsumenten werden im selben Schritt nachgezogen). `/mcp` **und** `/rpc`
   liefern die Spec-Hülle:
   - `structuredContent` trägt das unveränderte Domänen-Objekt `{data, provenance}`
     bzw. `{error, hint}` — der ADR-004-Provenance-Vertrag bleibt byte-gleich,
     nur eine Ebene tiefer.
   - `content[0]` trägt dieselbe Nutzlast als kompakt serialisierten JSON-Text
     (Spec-Empfehlung für Clients ohne `structuredContent`-Support).
   - `isError: true` für Tool-Execution-Fehler (lenkende `{error, hint}`-Hülle,
     Delta #13 aus ADR-008 bleibt in-band).
2. **`outputSchema` je Tool** in `tools/list`: zentral emittiert, weil es eine
   Architektur-Invariante spiegelt — **jede** Erfolgsantwort ist
   `{data, provenance}` mit exakt geformter Provenance (ADR-004/006:
   `kind: "norm"|"hint"`, `eli`, `valid_as_of`, `transaction_time`); `data`
   bleibt bewusst tool-spezifisch untypisiert.
3. **Konsumenten-Nachzug deploy-reihenfolge-sicher:** ansV und
   syllogismus-fedlex lesen `structuredContent` **bevorzugt** und fallen auf
   das rohe `result` zurück (gleiches Muster wie `inputSchema`/`schema` in
   ADR-008 §B-5). Damit sind Konsument-zuerst- und Server-zuerst-Deploys beide
   verträglich; der Fallback fällt mit dem Legacy-`schema`-Schlüssel in
   Runbook-Phase 9.

## Verworfene Alternativen

- **Dual-Format je Endpoint** (`/rpc` alt, `/mcp` neu): zwei Wire-Wahrheiten,
  doppelte Testfläche, und der Alt-Pfad bliebe für generische Clients kaputt.
- **`content[]` ohne `structuredContent`:** verschenkt getypte Antworten —
  gerade für Rechtsdaten mit fixem Provenance-Schema der eigentliche Gewinn.
- **Per-Tool-`outputSchema` mit exaktem `data`-Typ:** 25 handgepflegte Schemata
  ohne Wache wären die nächste Drift-Quelle; das zentrale Schema behauptet nur,
  was strukturell erzwungen ist.

## Konsequenzen

- Baseline-Tests verriegeln die neue Hülle (Provenance-Pfad:
  `result.structuredContent.provenance`).
- Audit-Log und Metriken lesen die Nutzlast aus `structuredContent`
  (`isError` steuert das `outcome`-Label).
- README-/Smoke-Beispiele zeigen die neue Antwortform; `scripts/smoke.sh`
  prüft den neuen Provenance-Pfad.
