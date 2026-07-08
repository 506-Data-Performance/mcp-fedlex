# Dokumentations-Index

Die Dokumentation ist in **zwei Ordner** getrennt, die verschiedene Fragen
beantworten:

| Ordner | Für wen | Frage |
|---|---|---|
| [`handbuch/`](./handbuch) — **Verständnis-Dokumentation** | Alle — ohne Vorwissen lesbar | «Was ist das, wie funktioniert es, warum kann ich ihm trauen?» |
| [`dev/`](./dev) — **Entwickler- & Betriebs-Dokumentation** | Entwickler:innen, Betrieb | «Wie ist es gebaut, konfiguriert, betrieben — und warum so?» |

**Konfliktregel:** Bei Widersprüchen gewinnen die technischen Dokumente (sie sind
quell-/testnah); das Handbuch wird nachgezogen.

## `handbuch/` — Verständnis-Dokumentation

- [`HANDBUCH.de.md`](./handbuch/HANDBUCH.de.md) 🇩🇪 — das Handbuch: von Grund auf
  erklärt, alle 40 Werkzeuge, FAQ, Glossar. Eigenständig lesbar, für den
  PDF-Export gedacht.
- [`HANDBUCH.en.md`](./handbuch/HANDBUCH.en.md) 🇬🇧 — englische Fassung
  (inhaltsgleich; Original ist die deutsche).

**PDF erzeugen** (benötigt [Pandoc](https://pandoc.org/) und eine
XeLaTeX-Installation, z. B. `brew install pandoc` + MacTeX/BasicTeX):

```bash
# Deutsch:
pandoc docs/handbuch/HANDBUCH.de.md -f markdown+gfm_auto_identifiers \
  -o mcp-fedlex-handbuch.de.pdf \
  --pdf-engine=xelatex --toc --toc-depth=1 \
  -V mainfont="Lucida Grande" -V monofont="Menlo" \
  -V geometry:margin=2.5cm -V lang=de -V colorlinks=true

# English:
pandoc docs/handbuch/HANDBUCH.en.md -f markdown+gfm_auto_identifiers \
  -o mcp-fedlex-handbook.en.pdf \
  --pdf-engine=xelatex --toc --toc-depth=1 \
  -V mainfont="Lucida Grande" -V monofont="Menlo" \
  -V geometry:margin=2.5cm -V lang=en -V colorlinks=true
```

(`gfm_auto_identifiers` lässt die internen Kapitel-Links im PDF funktionieren;
der Markdown-Reader berechnet Tabellen-Spaltenbreiten, damit breite Tabellen
umbrechen statt überzulaufen; die beiden Fonts decken die verwendeten
Unicode-Zeichen ab.)

## `dev/` — Entwickler- & Betriebs-Dokumentation

**Lexika (der Funktionsraum der Datenquellen)**

- [`dev/10_LEXICON_jolux.md`](./dev/10_LEXICON_jolux.md) — JOLux-Primitive
  (Metadaten/Graph), inkl. Betriebsregeln (Fedlex-WAF, Virtuoso-Datumsvergleiche).
- [`dev/11_LEXICON_akn.md`](./dev/11_LEXICON_akn.md) — AKN-Primitive (Volltext/Struktur).

**Planung, Status, Reviews (lebende Register)**

- [`dev/30_PLAN.md`](./dev/30_PLAN.md) — Umsetzungsplan & Checkliste.
- [`dev/40_FINDINGS.md`](./dev/40_FINDINGS.md) ·
  [`dev/45_GAP_ANALYSIS.md`](./dev/45_GAP_ANALYSIS.md) ·
  [`dev/50_ROADMAP_TO_PERFECT.md`](./dev/50_ROADMAP_TO_PERFECT.md) — Befunde, Lücken, Roadmap.
- [`dev/55_MIGRATION_mcp_protocol_upgrade.md`](./dev/55_MIGRATION_mcp_protocol_upgrade.md) —
  Runbook des MCP-Versionssprungs (abgeschlossen).
- [`dev/60_OPEN_ITEMS_AND_USABILITY.md`](./dev/60_OPEN_ITEMS_AND_USABILITY.md) — offene Punkte.
- [`dev/65_REVIEW_FINDINGS.md`](./dev/65_REVIEW_FINDINGS.md) (Code) ·
  [`dev/66_WEB_REVIEW_FINDINGS.md`](./dev/66_WEB_REVIEW_FINDINGS.md) (Website) ·
  [`dev/68_AGENT_UX_FINDINGS.md`](./dev/68_AGENT_UX_FINDINGS.md) (Agent-UX/Dogfooding).
- [`dev/67_HARDENING_AND_SOTA_ROADMAP.md`](./dev/67_HARDENING_AND_SOTA_ROADMAP.md) — Härtung.
- [`dev/IMPLEMENTATION_BRIEFING_lexicon_projection_matrix.md`](./dev/IMPLEMENTATION_BRIEFING_lexicon_projection_matrix.md)
  — Briefing zur Projektions-Matrix.

**Betrieb & Konfiguration**

- [`dev/70_CONFIG.md`](./dev/70_CONFIG.md) — alle Umgebungsvariablen (Referenz).
- [`dev/80_DEPLOY.md`](./dev/80_DEPLOY.md) — Produktion auf Kubernetes, mTLS, Runbooks.
- [`dev/90_AUTH_AND_ROLES.md`](./dev/90_AUTH_AND_ROLES.md) — Identitäts- und Rechtemodell.

**Entscheidungen**

- [`dev/adr/`](./dev/adr) — Architecture Decision Records (ADR-001 … ADR-011),
  Index in [`dev/adr/README.md`](./dev/adr/README.md).

**Ausserhalb von `docs/`:** `README.md` (Projekt-Einstieg, 4 Sprachen) ·
`CONTRIBUTING.md` (Dev-Anleitung, massgeblich) · `SECURITY.md` · `CHANGELOG.md` ·
`CLAUDE.md` (destillierte Invarianten) · `likec4/` (Architekturplan).
