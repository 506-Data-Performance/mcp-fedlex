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

---

## Triage-Regel (ein Satz)

Ein neues Finding kommt zuerst hierher (🔴), wird bewertet und wandert dann an seinen
Ziel-Ort: einplanbar ⇒ nach [60](60_OPEN_ITEMS_AND_USABILITY.md), entscheidungsbedürftig ⇒ ein
neuer ADR unter [adr/](adr/), sofort behebbar ⇒ Korrektur + Status 🟢 mit Beleg.
