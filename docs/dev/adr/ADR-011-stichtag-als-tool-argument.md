# ADR-011: Stichtag (`as_of`) als Tool-Argument — der Agenten-Kanal

- **Status:** Accepted — umgesetzt 2026-07-02
- **Datum:** 2026-07-02
- **Kontext-Artefakt:** `crates/mcp-reader/src/{transport,registry}.rs`, [68 §A-2](../68_AGENT_UX_FINDINGS.md)
- **Betrifft:** `tools/call`-Wire-Vertrag (additiv), Schema-Annonce aller Tools

## Kontext

Der Stichtag `as_of` wurde bisher ausschliesslich auf `params`-Ebene gelesen —
als Geschwister von `name`/`arguments`. Das war für die eigenen Clients (ansV,
mcp-fedlex-skills) gedacht, die den Stichtag applikationsseitig pinnen. Die
Dogfooding-Session (68 §A-2) zeigte die Konsequenz für alle anderen: Ein Modell
hinter einem Standard-MCP-Host füllt **nur** `arguments` — es konnte den
Stichtag **nie** setzen. Schlimmer: Ein `as_of` in den `arguments` wurde
stillschweigend ignoriert; der Agent glaubte, historisch gefragt zu haben, und
bekam heutiges Recht mit Norm-Provenance. `check_in_force` («war eine Norm zum
Stichtag in Kraft») hatte nicht einmal einen Datums-Parameter im Schema. Das
Kernversprechen «zeitpunktgenau» war für generische MCP-Agenten unerreichbar.

## Entscheidung

1. **`arguments.as_of` wird gelesen** (Format `JJJJ-MM-TT`, identische
   Validierung wie der params-Kanal; ungültige Werte sind derselbe in-band
   Tool-Error). Zentral im Transport — kein Tool muss den Stichtag selbst
   behandeln.
2. **Vorrang: `params.as_of` > `arguments.as_of`.** Ein Host, der den Stichtag
   pinnt, bleibt souverän gegenüber dem Modell. Bestehende Clients sind
   unberührt (additives Delta).
3. **Zentrale Schema-Annonce:** `tools/list` injiziert das optionale
   `as_of`-Property in jedes Tool-Schema. Zentral statt 40-fach von Hand, weil
   der Stichtag eine Dimension **jedes** Aufrufs ist — jede Antwort trägt
   `provenance.valid_as_of`. Ein Tool-eigenes `as_of`-Property hätte Vorrang
   (Entry-API), Drift ist ausgeschlossen.

## Warum das ADR-002/ADR-004 nicht verletzt

- **ADR-002 (Identität nie aus LLM-Parametern):** Der Stichtag ist keine
  Identität, sondern ein fachlicher Query-Parameter — dieselbe Kategorie wie
  `compare_to` von `compare_versions`, das seit jeher ein ISO-Datum in den
  `arguments` trägt. `tenant`/`session`/`role` kommen weiterhin nur aus
  geprüften Claims.
- **ADR-004 (Provenance serverseitig gestempelt):** Unverändert. Der Server
  stempelt das **effektiv verwendete** Datum in `provenance.valid_as_of`; ein
  Client kann die Provenance nicht fälschen, nur den Stichtag *wünschen* —
  und sieht im Stempel, was tatsächlich galt.

## Konsequenzen

- Agenten hinter Standard-Hosts erreichen die Zeitachse: `read_article`,
  `check_in_force`, `resolve_consolidation_at` etc. sind jetzt zum Stichtag
  befragbar.
- `instructions` (68 §A-3) erklärt den Kanal dem Modell.
- Abnahme: Transport-Tests (arguments-Kanal, Vorrangregel, in-band Error) +
  Baseline (jedes Tool annonciert optionales `as_of`, nie required).
