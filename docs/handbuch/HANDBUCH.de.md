# mcp-fedlex — Das Handbuch

🇩🇪 Deutsch · [🇬🇧 English](./HANDBUCH.en.md)

**Verständlich erklärt, von Grund auf.**

> **Was dieses Dokument ist.** Die Verständnis-Dokumentation von mcp-fedlex: Sie erklärt
> ohne Vorwissen, was das System ist, wie es funktioniert, warum man seinen Antworten
> trauen kann und wo seine Grenzen liegen — inklusive aller 40 Werkzeuge, einem grossen
> Fragen-und-Antworten-Teil und einem Glossar. Es ist als eigenständiges Dokument lesbar
> und für den PDF-Export gedacht (siehe [`docs/README.md`](../README.md)).
>
> **Was es nicht ist:** eine Entwickler-Referenz. Wer implementiert, konfiguriert oder
> betreibt, findet die massgeblichen technischen Dokumente im
> [Dokumentations-Index](../README.md).

Stand: 2026-07-08 · Software-Version: v0.2.0 · Lizenz: Apache-2.0 © [mindful.bio](https://mindful.bio)

---

**Inhalt**

*Teil I — Verstehen*

- [1. Worum geht es?](#1-worum-geht-es)
- [2. Die Grundbegriffe, einfach erklärt](#2-die-grundbegriffe-einfach-erklärt)
- [3. Wie eine Anfrage abläuft](#3-wie-eine-anfrage-abläuft)
- [4. Beleg oder Hinweis — das Herzstück](#4-beleg-oder-hinweis--das-herzstück)
- [5. Der Stichtag — die eingebaute Zeitmaschine](#5-der-stichtag--die-eingebaute-zeitmaschine)

*Teil II — Die Werkzeuge*

- [6. Vier Werkzeugkästen, drei Rollen](#6-vier-werkzeugkästen-drei-rollen)
- [7. Alle 40 Werkzeuge im Einzelnen](#7-alle-40-werkzeuge-im-einzelnen)
- [8. Typische Arbeitsabläufe](#8-typische-arbeitsabläufe)

*Teil III — Vertrauen, Sicherheit, Grenzen*

- [9. Warum man den Antworten trauen kann](#9-warum-man-den-antworten-trauen-kann)
- [10. Wer darf was — Identität und Rollen](#10-wer-darf-was--identität-und-rollen)
- [11. Die Mengenbegrenzung (Quota)](#11-die-mengenbegrenzung-quota)
- [12. Datenschutz und Protokollierung](#12-datenschutz-und-protokollierung)
- [13. Die Grenzen des Systems — ehrlich benannt](#13-die-grenzen-des-systems--ehrlich-benannt)

*Teil IV — Selbst ausprobieren*

- [14. In zwei Minuten lokal starten](#14-in-zwei-minuten-lokal-starten)
- [15. Im Browser durchklicken (MCP Inspector)](#15-im-browser-durchklicken-mcp-inspector)
- [16. An einen KI-Client anschliessen](#16-an-einen-ki-client-anschliessen)
- [17. Betrieb in Produktion — der Überblick](#17-betrieb-in-produktion--der-überblick)

*Teil V — Nachschlagen*

- [18. Fragen und Antworten (FAQ)](#18-fragen-und-antworten-faq)
- [19. Glossar](#19-glossar)
- [20. Weiterführende Dokumente](#20-weiterführende-dokumente)

---

# Teil I — Verstehen

## 1. Worum geht es?

**Das Problem.** Sprachmodelle («KI», z. B. hinter Chat-Assistenten) formulieren
Antworten aus dem, was sie beim Training gelernt haben. Bei Rechtsfragen ist das
gefährlich: Das Modell klingt überzeugend, kann aber Artikel erfinden, veraltete
Fassungen wiedergeben oder Gesetze verwechseln — und niemand sieht der Antwort an,
woher sie stammt. Für juristische Arbeit ist eine Antwort ohne überprüfbare Quelle
wertlos.

**Die Lösung.** mcp-fedlex ist ein Server, der einem Sprachmodell **Werkzeuge** in
die Hand gibt, um im Schweizer Bundesrecht **nachzuschlagen statt zu erfinden**.
Man kann ihn sich als penibel arbeitenden Bibliothekar vorstellen:

- Er holt den **echten Gesetzestext** aus Fedlex, der amtlichen Publikationsplattform
  des Bundesrechts — nicht aus dem Gedächtnis des Modells.
- Er liefert jeden Text **zu einem bestimmten Stichtag** («Was galt am 1. Januar 2019?»),
  nicht einfach «irgendeine Fassung».
- Er heftet an **jede Antwort einen Herkunftsnachweis** (welcher Erlass, welcher
  Stichtag), den das Modell nicht fälschen kann.
- Er unterscheidet strukturell zwischen einem **Beleg** (das ist der Normtext) und
  einem **Hinweis** (das könnte relevant sein — bitte prüfen).

**In einem Satz:** mcp-fedlex macht Schweizer Bundesrecht für KI-Systeme so zugänglich,
dass jede Aussage **zitierfähig, stichtagsgenau und nachprüfbar** ist.

**Was es bewusst nicht ist:** kein Chatbot, keine Rechtsberatung, keine eigene
Rechtsdatenbank. Es ist die kontrollierte Brücke zwischen einer KI und den öffentlichen,
amtlichen Fedlex-Daten.

**Wer steckt dahinter?** mcp-fedlex ist ein Produkt der Firma
[mindful.bio](https://mindful.bio). Es ist der Daten-Layer einer kleinen Produktfamilie:
Die Anwendungsplattform **[ansV](https://ansv.ch)** setzt darauf auf und erstellt
juristische Analysen mit nachvollziehbarer Belegkette; die fünfsprachige
Projektbeschreibung steht auf **[mcp-fedlex.ch](https://mcp-fedlex.ch)**. Der Quellcode
ist unter der Apache-2.0-Lizenz offen.

## 2. Die Grundbegriffe, einfach erklärt

Diese acht Begriffe genügen, um alles Weitere zu verstehen.

**Fedlex** — die offizielle Online-Publikationsplattform des Schweizer Bundesrechts,
betrieben vom Bund. Dort stehen alle Bundesgesetze, Verordnungen und Staatsverträge —
öffentlich und kostenlos. mcp-fedlex erfindet keine Daten, es liest ausschliesslich
von dort.

**Erlass** — der Sammelbegriff für ein einzelnes «Stück» Recht: ein Gesetz, eine
Verordnung, ein Staatsvertrag. Beispiel: die Bundesverfassung ist ein Erlass, das
Obligationenrecht ein anderer.

**SR-Nummer** — die «Regalnummer» der Systematischen Rechtssammlung, mit der
Jurist:innen Erlasse zitieren. Beispiel: SR 101 ist die Bundesverfassung, SR 220
das Obligationenrecht. Wichtig: SR-Nummern werden über die Jahrzehnte
**wiederverwendet** — ein aufgehobener und ein geltender Erlass können dieselbe
Nummer tragen (deshalb behandelt mcp-fedlex SR-Auflösungen als Hinweise, nicht als
Belege; siehe Kapitel 4).

**ELI** — der *European Legislation Identifier*: die eindeutige, dauerhafte
Web-Adresse eines Erlasses. Beispiel: `eli/cc/1999/404` ist die Bundesverfassung.
Während SR-Nummern mehrdeutig sein können, ist ein ELI eindeutig — darum arbeitet
mcp-fedlex intern immer mit ELIs und weist sie in jedem Herkunftsnachweis aus.

**Konsolidierte Fassung und Stichtag** — Gesetze ändern sich laufend. Eine
*konsolidierte Fassung* ist der Gesetzestext mit allen bis zu einem bestimmten Datum
eingearbeiteten Änderungen — so, wie das Gesetz an diesem Tag galt. Dieses Datum heisst
**Stichtag**. Jede Antwort von mcp-fedlex bezieht sich auf genau einen Stichtag
(ohne Angabe: heute).

**MCP (Model Context Protocol)** — ein offener Standard, über den KI-Anwendungen
externe Werkzeuge nutzen. Man kann sich MCP wie einen genormten Steckdosen-Standard
vorstellen: Jede kompatible KI-Anwendung (z. B. Claude Desktop und viele andere) kann
sich mit einem MCP-Server verbinden und dessen Werkzeuge benutzen — ohne
Spezialanpassung.

**Tool (Werkzeug)** — eine einzelne, klar umrissene Fähigkeit, die der Server der KI
anbietet, z. B. «lies Artikel X des Erlasses Y» (`read_article`) oder «suche Gesetze
zum Stichwort Z» (`search_law`). mcp-fedlex bietet 40 solcher Werkzeuge an
(alle in Kapitel 7).

**Die zwei Datenquellen: JOLux und AKN** — Fedlex stellt seine Daten in zwei Formen
bereit, und mcp-fedlex nutzt beide für das, was sie am besten können:

| | **JOLux** (Metadaten-Graph) | **AKN** (Volltext-Dokumente) |
|---|---|---|
| Was es ist | Ein «Wissensnetz» über alle Erlasse: Titel, Daten, Fassungen, wer ändert wen, wer zitiert wen | Der eigentliche Gesetzestext als strukturiertes XML-Dokument (Standard «Akoma Ntoso») |
| Beantwortet | «Gibt es …? Seit wann? In welcher Fassung? Was hängt zusammen?» | «Was steht drin — Wort für Wort, Artikel für Artikel?» |
| Zugriff | live per Abfragesprache SPARQL auf den öffentlichen Fedlex-Endpunkt | Dokument wird einmal geholt und dann aus dem Zwischenspeicher bedient |

Merkregel: **JOLux liefert nie Gesetzestext, AKN liefert nie Metadaten** — die
Werkzeuge kombinieren beide Welten und verstecken diese Arbeitsteilung vor der KI.

## 3. Wie eine Anfrage abläuft

Was passiert, wenn eine KI z. B. «Art. 8 der Bundesverfassung, Stand 1.1.2024» lesen
will? Jeder einzelne Aufruf durchläuft dieselbe Kette — keine Abkürzungen, keine
Ausnahmen:

```
KI-Anwendung (MCP-Client)
      │  Aufruf: read_article(eli, art_8), Stichtag 2024-01-01
      ▼
┌─────────────────────────────────────────────────────────────┐
│  mcp-fedlex Server («Reader»)                               │
│                                                             │
│  1. Ausweiskontrolle    Wer ruft an? Token prüfen.          │
│     (Auth)              Ohne gültigen Ausweis: Abbruch.     │
│  2. Berechtigung        Darf diese Rolle dieses Werkzeug    │
│     (RBAC)              überhaupt sehen und benutzen?       │
│  3. Kontingent          Ist das Nutzungs-Budget dieser      │
│     (Quota)             Sitzung noch nicht aufgebraucht?    │
│  4. Werkzeug            Stichtags-Fassung ermitteln,        │
│     ausführen           Text holen (Cache oder Fedlex).     │
│  5. Herkunfts-Stempel   ELI + effektiver Stichtag werden    │
│     (Provenance)        SERVERSEITIG in die Antwort         │
│                         gestempelt.                         │
└─────────────────────────────────────────────────────────────┘
      │  Antwort: Normtext + Herkunftsnachweis
      ▼
KI-Anwendung → zitiert: «Art. 8 BV, Fassung vom 01.01.2024»
      ▲
      └── Datenquelle dahinter: Fedlex (SPARQL-Endpunkt + XML-Ablage)
```

Drei Eigenschaften dieser Kette sind entscheidend:

- **Die Reihenfolge ist fix.** Erst Ausweis, dann Berechtigung, dann Kontingent, dann
  Arbeit. Ein nicht ausgewiesener Aufruf erreicht nie ein Werkzeug.
- **Fail-closed.** Bei jedem Zweifel (ungültiges Token, unbekannte Rolle, Ausfall der
  Kontingent-Datenbank) verweigert der Server — er «lässt nicht im Zweifel durch».
- **Der Stempel kommt vom Server.** Die KI kann sich einen Stichtag *wünschen*, aber
  was tatsächlich galt und verwendet wurde, schreibt allein der Server in die Antwort.

## 4. Beleg oder Hinweis — das Herzstück

Die vielleicht wichtigste Design-Entscheidung von mcp-fedlex: Jede Antwort trägt eine
von zwei **strukturell unterscheidbaren** Herkunfts-Sorten.

**Beleg (`kind: "norm"`)** — «Das ist der Inhalt/Zustand eines konkreten, benannten
Erlasses zum genannten Stichtag.» Beispiele: der Text eines Artikels
(`read_article`), die Antwort auf «war dieser Erlass am 1.1.2019 in Kraft?»
(`check_in_force`). Belege darf ein KI-System als Zitate verwenden.

**Hinweis (`kind: "hint"`)** — «Das könnte relevant sein — es ist ein Kandidat, kein
Zitat.» Beispiele: Treffer einer Gesetzessuche (`search_law`), die Auflösung einer
SR-Nummer (`resolve_sr_number`, mehrdeutig!), Vernehmlassungsunterlagen
(Entstehungskontext, kein geltendes Recht).

Warum ist das so wichtig? Ein KI-System, das recherchiert, findet zuerst *Kandidaten*
(«diese fünf Gesetze könnten passen») und liest dann nach («in diesem steht
tatsächlich …»). Wenn beide Schritte gleich aussehen, passiert der klassische Fehler:
Ein Suchtreffer wird als belegte Norm verbucht. mcp-fedlex macht diesen Fehler
**strukturell unmöglich** — Beleg und Hinweis sind im Datenformat verschieden, nicht
nur in einer Textbeschreibung. Der korrekte Arbeitsablauf ist immer:
**Hinweis finden → mit einem Beleg-Werkzeug nachlesen → erst dann zitieren.**

## 5. Der Stichtag — die eingebaute Zeitmaschine

Recht ist zeitabhängig: Die Frage «Was sagt das Gesetz?» ist unvollständig ohne
«… und wann?». Deshalb ist der Stichtag in mcp-fedlex keine Zusatzfunktion, sondern
eine Dimension **jedes** Aufrufs:

- **Jedes Werkzeug** akzeptiert einen optionalen Parameter `as_of` im Format
  `JJJJ-MM-TT` (z. B. `2019-01-01`). Ohne Angabe gilt **heute** (Schweizer Zeit).
- Der Server ermittelt die zum Stichtag **gültige konsolidierte Fassung** — nie
  stillschweigend «die neueste».
- Das **effektiv verwendete Datum** steht immer im Herkunftsnachweis der Antwort
  (`provenance.valid_as_of`) — serverseitig gestempelt, von keinem Aufruf-Parameter
  fälschbar.
- Der Stichtag kann auf zwei Wegen gesetzt werden: von der KI selbst (im normalen
  Werkzeug-Aufruf — jedes Werkzeug annonciert den Parameter) oder **fest verdrahtet
  von der Host-Anwendung**; die Host-Vorgabe hat Vorrang. So kann eine Anwendung
  z. B. eine ganze Analyse auf den 31.12.2023 «pinnen», ohne dass das Modell das
  ändern kann.
- Sonderfall aufgehobene Erlasse: Liest man einen zum Stichtag bereits aufgehobenen
  Erlass, liefert der Server den Text trotzdem (letzte Fassung), markiert die Antwort
  aber mit dem Aufhebungsdatum (`repealed_since`) — man sieht also unübersehbar:
  **Das ist kein geltendes Recht mehr.**

---

# Teil II — Die Werkzeuge

## 6. Vier Werkzeugkästen, drei Rollen

Die 40 Werkzeuge sind in vier **Pools** («Werkzeugkästen») gruppiert. Der Pool
bestimmt zwei Dinge: *wer* die Werkzeuge sieht (Rollen) und *was ein Aufruf kostet*
(Kontingent, Kapitel 11).

| Pool | Werkzeuge | Was er kann | Datenweg | Kosten pro Aufruf |
|---|---|---|---|---|
| **LocalNavigation** | 13 | Im Text eines bekannten Erlasses lesen und navigieren | Zwischenspeicher (Dokument wird einmal geholt) | 1 |
| **Discovery** | 10 | Erlasse, Verträge, Vokabulare **finden** — Ergebnisse sind Hinweise | live zu Fedlex | 5 |
| **JoluxMetadata** | 16 | Metadaten und Beziehungen eines **bekannten** Erlasses — Ergebnisse sind Belege | live zu Fedlex | 5 |
| **Validation** | 1 | Fassungen vergleichen | Zwischenspeicher | 1 |

Dazu kommen drei **Rollen**, streng ineinander geschachtelt (jede höhere Rolle kann
alles, was die niedrigere kann):

| Rolle | Sieht Pools | Anzahl Werkzeuge | Typischer Nutzer |
|---|---|---|---|
| **Reader** | LocalNavigation | 13 | Reine Lese-Integration |
| **Navigator** | + Discovery, JoluxMetadata | 39 | Recherche-Anwendungen — so läuft ansV |
| **Validator** | + Validation | 40 | Prüf- und Vergleichsarbeit |

Die Rolle steckt im geprüften Zugangs-Token (Kapitel 10) — eine KI kann sie weder
wählen noch hochstufen. `tools/list` zeigt jeder Rolle nur ihre erlaubten Werkzeuge,
und selbst wer einen fremden Werkzeugnamen «errät», wird beim Aufruf abgewiesen.

> Im Code sind zwei weitere Pools (`LodFederation`, `Workspace`) für künftige
> Werkzeuge reserviert, aber leer — daher überall die Formulierung «vier **aktive**
> Pools».

## 7. Alle 40 Werkzeuge im Einzelnen

Jede Zeile: Werkzeugname, die Frage, die es beantwortet, und was man wissen muss.
(«Beleg»/«Hinweis» bezieht sich auf Kapitel 4.)

### Pool LocalNavigation — im Erlasstext lesen (13 Werkzeuge, Beleg — eine Ausnahme)

| Werkzeug | Beantwortet die Frage | Wissenswert |
|---|---|---|
| `read_article` | «Was steht in Artikel X zum Stichtag?» | Das Brot-und-Butter-Werkzeug. Bei aufgehobenen Erlassen trägt die Antwort das Aufhebungsdatum (`repealed_since`). |
| `read_element` | «Was steht in Kapitel/Ebene/Element X?» | Für Erlasse, die nicht in Artikel gegliedert sind. |
| `read_document` | «Gib mir den ganzen Erlass als lesbaren Text.» | Liefert Markdown mit Zeichen-Budget (Standard 120 000), damit kein KI-Kontext «gesprengt» wird; Fortsetzung über einen Offset. |
| `get_structure` | «Wie ist der Erlass gegliedert?» (Inhaltsverzeichnis) | Standard: Skelett bis Artikel-Ebene; auf Wunsch kompletter Baum oder flache Artikelliste. |
| `search_text` | «Wo im Erlass kommt Begriff X vor?» | Sucht nur **innerhalb eines** Erlasses; meldet Gesamttrefferzahl und ob gekappt wurde. Kein Ersatz für eine semantische Suche. |
| `get_metadata` | «Welches Werk, welche Fassung, welche Sprache ist das — und wie ist es aufgebaut?» | Sinnvoll **vor** `read_article`: 91 % der Änderungserlasse/Bundesblatt-Dokumente haben gar keine Artikel. |
| `get_references` | «Auf welche anderen Erlasse verweist der Text?» | Paginierte Liste der Verweise im Dokument. |
| `get_modifications` | «Welche Änderungsanweisungen enthält dieser Änderungserlass — mit neuem Wortlaut?» | Bei konsolidierten Fassungen leer (dort sind Änderungen bereits eingearbeitet). |
| `list_components` | «Welche Anhänge/Beilagen hat der Erlass?» | Anhänge sind eigenständige Werke mit eigener Identität; Leer-Anhänge sind markiert. |
| `extract_tables` | «Welche Tabellen enthält der Erlass?» | Tarife, Grenzwerte, Zuständigkeits-Matrizen — als strukturierte Einheiten, optional auf einen Teilbaum begrenzt. |
| `detect_foreign_content` | «Enthält der Erlass eingebettete Grafiken oder Formeln?» | Selten, aber juristisch relevant (z. B. Berechnungsformeln in Verordnungen). |
| `extract_change_notes` | «Welche redaktionellen Änderungsnotizen (Fussnoten zu AS-Änderungen) trägt der Text?» | Zeigt die dokumentierte Änderungshistorie im Text selbst. |
| `parse_unlinked_ref` | «Was bedeutet der Verweistext ‹Art. 58 Abs. 1 ParlG›?» | Reiner Text-Parser, darum die Ausnahme im Pool: Ergebnis ist ein **Hinweis** — anschliessend mit `read_article`/`get_metadata` belegen. |

### Pool Discovery — Erlasse finden (10 Werkzeuge, alle Hinweis)

| Werkzeug | Beantwortet die Frage | Wissenswert |
|---|---|---|
| `search_law` | «Welche Erlasse passen zu Stichwort/Kürzel/Volksname X?» | Amtliche Kürzel (OR, ZGB, DSG …) werden exakt aufgelöst und stehen zuerst; Treffer zeigen, ob sie **zum Stichtag** in Kraft waren; blätterbar. |
| `resolve_sr_number` | «Welcher Erlass steckt hinter SR-Nummer X?» | Liefert bewusst **mehrere** Kandidaten — SR-Nummern werden wiederverwendet; über «in Kraft zum Stichtag» unterscheiden. |
| `find_related_topic` | «Welche Erlasse gehören zum selben Rechtsgebiet?» | Deterministische Navigation über die amtliche Rechtstaxonomie. |
| `find_treaties` | «Welche Staatsverträge gibt es (mit Land X / bilateral)?» | Filter nach Vertragspartner und Bilateralität. |
| `get_treaty_info` | «Details zu diesem Staatsvertrags-Prozess?» | Titel, Partner, Daten, Status. |
| `get_consultations` | «Welche Vernehmlassungen gab es zu diesem Entwurf?» | Entstehungskontext — per Definition kein geltendes Recht. |
| `get_consultation_documents` | «Welche Berichte/Stellungnahmen gehören zur Vernehmlassung?» | dito. |
| `resolve_vocabulary_label` | «Was bedeutet diese Vokabular-URI in Sprache X?» | Nachschlagewerk für kodierte Werte aus anderen Antworten. |
| `list_vocabulary` | «Welche Konzepte enthält Vokabular X?» (z. B. Länderliste) | Mit Suchbegriff gezielt filterbar — z. B. «Deutschland» → Land-URI für `find_treaties`. |
| `explore_node` | «Was hängt im Fedlex-Datennetz an diesem Knoten?» | Experten-Werkzeug: zeigt ein- und ausgehende Kanten eines beliebigen Knotens. |

### Pool JoluxMetadata — Metadaten & Beziehungen (16 Werkzeuge, alle Beleg)

| Werkzeug | Beantwortet die Frage | Wissenswert |
|---|---|---|
| `check_in_force` | «War dieser Erlass zum Stichtag in Kraft?» | Achtung, zwei Zeitbezüge: `in_force` gilt **zum Stichtag**; der mitgelieferte Vokabular-Status ist immer der **heutige**. |
| `list_versions` | «Welche Fassungen gab es — chronologisch?» | Vollständige Liste; eine leere Liste heisst «keine Konsolidierungen», nicht «Fehler». |
| `resolve_consolidation_at` | «Welche Fassung galt am Tag X — und wo liegt ihr XML?» | Das Werkzeug hinter der Stichtags-Genauigkeit; meldet sauber «keine Fassung zum Stichtag», statt die neueste zu liefern. |
| `get_impacts` | «Welche Änderungen wirkten auf diesen Erlass?» | Wichtiger Vorbehalt: Seit 2023 nennt Fedlex betroffene Artikel oft nur im Freitext — eine leere Liste beweist **nicht** «nie geändert». |
| `get_outgoing_impacts` | «Welche Gesetze ändert dieser Änderungserlass?» | Gegenrichtung zu `get_impacts`; Mantelerlasse bündeln viele Ziele. |
| `get_article_history` | «Welche Änderungen trafen genau diesen Artikel?» | Trägt denselben Vollständigkeits-Vorbehalt direkt in der Antwort. |
| `get_citations` | «Wer zitiert diesen Erlass — und wen zitiert er?» | Nur auf Ebene ganzer Erlasse (nicht artikelgenau); Richtungen wählbar. |
| `get_taxonomy` | «In welche Rechtsgebiete ist der Erlass eingeordnet?» | Rund 10 000 Erlasse sind unklassifiziert — leere Liste ist normal. |
| `get_subdivisions` | «Welche Untergliederungen kennt der Metadaten-Graph?» | **Lückenkatalog, kein Inhaltsverzeichnis** — der Graph kennt nur Elemente mit mindestens einer Änderung; die Vollstruktur liefert `get_structure`. |
| `list_annexes` | «Welche Anhänge kennt der Metadaten-Graph?» | JOLux-Sicht (nur Anhänge mit Änderungen); komplementär zu `list_components`. |
| `get_law_metadata` | «Der Steckbrief: Titel, Kürzel, SR-Nummer, Status, Daten?» | Die kompakte Visitenkarte eines Erlasses. |
| `list_expressions` | «In welchen Sprachen existiert diese Fassung?» | Liefert Sprachcodes (de/fr/it/en/rm) — vor `read_article` prüfen, ob z. B. Rätoromanisch existiert. |
| `get_oc_act` | «Wo wurde der Erlass in der Amtlichen Sammlung (AS) publiziert?» | Einstieg in die amtliche Publikationskette. |
| `get_memorial` | «Band/Heft/Seiten der AS-Publikation?» | Die klassische Fundstellen-Angabe. |
| `get_fga_documents` | «Welche Bundesblatt-Dokumente (z. B. Botschaften) gehören dazu?» | Materialien für Auslegung und Entstehungsgeschichte. |
| `get_drafts` | «Welche Gesetzgebungs-Entwürfe gehören zu diesem Erlass?» | Einstieg in die Entstehungsgeschichte; führt weiter zu `get_consultations`. |

### Pool Validation — Fassungen vergleichen (1 Werkzeug, nur Rolle Validator)

| Werkzeug | Beantwortet die Frage | Wissenswert |
|---|---|---|
| `compare_versions` | «Was hat sich zwischen Stichtag A und Stichtag B geändert?» | Liefert hinzugefügte, entfernte und geänderte Artikel als lesbares Destillat. |

## 8. Typische Arbeitsabläufe

Vier Beispiele, wie eine KI die Werkzeuge korrekt kombiniert:

**«Was sagt Art. 8 der Bundesverfassung?»**
1. ELI ist bekannt (`eli/cc/1999/404`) → direkt `read_article` mit `eid: art_8`.
2. Antwort trägt Text + Herkunftsnachweis → zitierfähig.

**«Galt diese Bestimmung auch am 1. Januar 2019?»**
1. `check_in_force` mit Stichtag `2019-01-01` → war der Erlass in Kraft? (Beleg)
2. `read_article` mit Stichtag `2019-01-01` → der damalige Wortlaut. (Beleg)
3. Optional `compare_versions` (Rolle Validator) → was sich seither geändert hat.

**«Finde das massgebliche Gesetz zum Thema Datenschutz.»**
1. `search_law` mit «Datenschutz» oder Kürzel «DSG» → Kandidaten. (Hinweise!)
2. Disambiguieren: Welcher Kandidat war zum Stichtag in Kraft?
3. `get_law_metadata` + `read_article` auf den gewählten ELI → jetzt erst Belege.

**«Der Text erwähnt ‹Art. 58 Abs. 1 ParlG› — was ist das?»**
1. `parse_unlinked_ref` zerlegt den Verweistext → strukturierter Kandidat. (Hinweis)
2. `search_law` mit dem Kürzel «ParlG» → ELI des Parlamentsgesetzes. (Hinweis)
3. `read_article`/`read_element` auf dem ELI → der belegte Wortlaut. (Beleg)

---

# Teil III — Vertrauen, Sicherheit, Grenzen

## 9. Warum man den Antworten trauen kann

Vier Mechanismen greifen ineinander — alle serverseitig, alle ausserhalb der
Kontrolle des Sprachmodells:

1. **Herkunftsnachweis per Konstruktion.** Jede Antwort trägt einen
   `provenance`-Block mit dem ELI des Erlasses und dem effektiv verwendeten Stichtag.
   Diesen Block erzeugt der Server; kein Aufruf-Parameter kann ihn setzen oder
   überschreiben.
2. **Beleg vs. Hinweis** (Kapitel 4). Suchtreffer können strukturell nie als Zitate
   verbucht werden.
3. **Identität aus dem Ausweis, nie aus dem Gespräch** (Kapitel 10). Mandant, Sitzung
   und Rolle stammen ausschliesslich aus dem geprüften Zugangs-Token. Selbst wenn ein
   Sprachmodell behauptet «ich bin Administrator», ändert das nichts.
4. **Fail-closed als Grundhaltung.** Ungültiger Ausweis → Ablehnung. Unbekannte
   Rolle im Token → Ablehnung (kein stilles Herabstufen). Kontingent-Datenbank
   ausgefallen → enges Notfall-Kontingent statt «freie Fahrt».

Dazu kommt Nachvollziehbarkeit über die Zeit: Versionierte Server-Releases sind an
Git-Tags gebundene, unveränderliche Docker-Images — eine Analyse kann festhalten,
mit **welcher Serverversion** sie erstellt wurde, und die meldet sich im Protokoll
selbst (`serverInfo.version`).

## 10. Wer darf was — Identität und Rollen

**Der Ausweis.** Jede Anfrage (ausser reinen Protokoll-Notifikationen) muss ein
**Bearer-Token** im `Authorization`-Header tragen. In Produktion ist das ein
**JWT** — ein signiertes «digitales Ticket» eines Identity-Providers, das der Server
kryptografisch prüft. Im Token stehen die Claims:

| Claim | Bedeutung |
|---|---|
| `iss` / `aud` / `exp` | Aussteller, Zielgruppe, Ablaufzeit — Standard-JWT-Prüfungen |
| `tenant` | Der Mandant (z. B. eine Kanzlei) — pseudonyme Audit-Identität |
| `sid` | Die Sitzung — pseudonyme Audit-Identität |
| `role` | `reader`, `navigator` oder `validator` |

Ein Token mit unbekannter Rolle wird abgewiesen. Für die lokale Entwicklung gibt es
ersatzweise ein statisches Dev-Token (niemals in Produktion).

**Mandantentrennung.** Alles, was der Server pro Aufruf durchsetzt und protokolliert
(Berechtigung, Kontingent, Audit), hängt am Paar `(tenant, session)` aus dem Token.
Zwei Organisationen, die denselben Server nutzen, teilen sich daher weder Kontingent
noch Audit-Spur — und keine kann im Namen der anderen agieren.

**Warum das wichtig ist:** In klassischen Systemen ist «der Benutzer» ein Mensch.
Hier sitzt zwischen Mensch und Server ein Sprachmodell, das Text generiert — auch
Texte wie «tenant: andere-kanzlei». Deshalb die eiserne Regel: **Identität kommt nie
aus einem Werkzeug-Parameter**, ausschliesslich aus dem geprüften Token.

## 11. Die Mengenbegrenzung (Quota)

Der Server begrenzt, wie viel jede Sitzung abrufen darf — aus zwei Gründen:
Schutz des **öffentlichen Fedlex-Endpunkts** (ein Gemeingut, das nicht von einer
heisslaufenden KI geflutet werden soll) und Fairness zwischen Mandanten.

Das Modell ist ein «Token-Bucket» — ein Eimer mit Guthaben, der stetig nachgefüllt
wird; jeder Aufruf schöpft daraus:

| Rolle | Eimergrösse (Kapazität) | Nachfüllrate |
|---|---|---|
| Reader | 60 | 1 pro Sekunde |
| Navigator | 120 | 2 pro Sekunde |
| Validator | 240 | 4 pro Sekunde |

Die **Kosten pro Aufruf** hängen am Pool, nie an einem Parameter der KI: Lesen aus
dem Zwischenspeicher kostet 1, Live-Abfragen zu Fedlex (Discovery, JoluxMetadata)
kosten 5. Die Buchhaltung läuft über eine zentrale Redis-Datenbank und gilt damit
**über alle Server-Instanzen hinweg** — mehr Pods bedeuten nicht mehr Kontingent.
Fällt diese Datenbank aus, gilt ein enges Notfall-Kontingent (Kapazität 5):
fail-closed, nicht fail-open.

Für die Praxis: Wer gebremst wird, hat kurzzeitig zu viel abgerufen — nach wenigen
Sekunden füllt sich der Eimer von selbst wieder.

## 12. Datenschutz und Protokollierung

**Was protokolliert wird.** Jeder Werkzeug-Aufruf erzeugt genau eine Audit-Zeile:
Mandant und Sitzung (pseudonym), Rolle, Werkzeugname, betroffener Erlass (ELI),
Stichtag, Ergebnis-Status und Dauer. Beispiel:

```json
{"event":"tools/call","tool.name":"read_article","auth.role":"Navigator",
 "auth.tenant":"kanzlei-a","auth.session":"sess-1",
 "provenance.eli":"eli/cc/1999/404","provenance.valid_as_of":"2024-01-01",
 "outcome":"ok","span.duration_ms":"29"}
```

**Was nie protokolliert wird.** Die rohen Aufruf-Argumente (z. B. Suchbegriffe, die
Rückschlüsse auf einen Mandanten-Fall erlauben könnten) und die Antwortinhalte. Das
erzwingt ein «PII-Scrubber» mit **Allowlist-Prinzip**: Nur ausdrücklich freigegebene,
pseudonyme Felder erreichen das Log — alles andere wird fail-closed redigiert. Ein
neues Feld ist also standardmässig *nicht* im Log, statt versehentlich doch.

So bleibt der Betrieb vollständig auditierbar («wer hat wann welchen Erlass zu
welchem Stichtag abgefragt»), ohne dass Inhalte oder Personendaten im Log landen.

## 13. Die Grenzen des Systems — ehrlich benannt

Ein vertrauenswürdiges System benennt, was es **nicht** kann:

- **Nur Bundesrecht.** Kantonales und kommunales Recht, Rechtsprechung (Urteile) und
  Literatur sind nicht enthalten — Fedlex publiziert Bundesrecht.
- **Keine Rechtsberatung.** mcp-fedlex liefert belegte Rohstoffe (Normtexte,
  Metadaten). Auslegung, Abwägung und Verantwortung bleiben bei Menschen.
- **Maschinenlesbarer Volltext erst ab ca. 2021.** Ältere konsolidierte Fassungen
  existieren bei Fedlex oft nur als PDF — die Volltext-Werkzeuge reichen dann nicht
  beliebig weit zurück. Die Metadaten (Fassungsliste, Änderungen, Daten) reichen
  deutlich weiter.
- **Lücken in den Metadaten sind normal.** Der Fedlex-Graph garantiert kaum ein Feld:
  Rund 10 000 Erlasse sind keinem Rechtsgebiet zugeordnet; seit 2023 stehen betroffene
  Artikel von Änderungen oft nur im Freitext. Die Werkzeuge sagen das in ihren
  Antworten ausdrücklich («leere Liste beweist nicht …») — eine leere Antwort ist
  ein Datum, kein Fehler.
- **Ein Drittel der Dokumente sind Metadaten-Hüllen.** Viele XML-Dateien (v. a.
  Änderungserlasse) haben keinen Textkörper; `get_metadata` erkennt das, bevor man
  ins Leere liest.
- **Abhängigkeit vom Fedlex-Betrieb.** Fällt der öffentliche Fedlex-Endpunkt aus,
  funktionieren Discovery/Metadaten-Werkzeuge nicht; bereits zwischengespeicherte
  Texte bleiben lesbar. Der Server meldet sich dann als «degraded», bleibt aber
  bewusst am Netz. Zudem sitzt vor Fedlex eine Firewall des Bundes, die in seltenen
  Fällen auch legitime Abfragen blockt — die Abfragen des Servers sind darauf
  ausgelegt, und ein regelmässiger Live-Konformanztest dient als Frühwarnsystem.
- **Suchen heisst finden, nicht verstehen.** `search_text` ist eine wörtliche Suche,
  `search_law` eine Titel-/Kürzel-Suche. Eine semantische Suche («finde sinngemäss…»)
  ist bewusst nicht Teil dieses Servers (dafür gibt es im Ökosystem eine separate
  Komponente).

---

# Teil IV — Selbst ausprobieren

## 14. In zwei Minuten lokal starten

Voraussetzung: [Docker](https://www.docker.com/) mit Compose. Eine
Rust-Entwicklungsumgebung ist **nicht** nötig.

```bash
git clone <Repository-URL> && cd mcp-fedlex
cp .env.example .env          # Standardwerte reichen zum Testen
docker compose up --build     # startet Reader + Redis
```

Der Server lauscht danach auf `http://localhost:8080`. Prüfen, ob er lebt:

```bash
curl -s http://localhost:8080/livez    # -> "ok"
curl -s http://localhost:8080/readyz   # prüft auch Redis + Fedlex-Erreichbarkeit
```

Und die erste echte Anfrage — Artikel 1 der Bundesverfassung, Stand 1.1.2024
(das Token ist das Dev-Token aus der `.env`):

```bash
TOKEN=dev-secret-change-me
curl -s -X POST http://localhost:8080/rpc \
  -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{
          "name":"read_article",
          "arguments":{"eli":"eli/cc/1999/404","eid":"art_1"},
          "as_of":"2024-01-01"
        }
      }' | jq
```

In der Antwort sieht man beides: den Normtext **und** den Herkunftsnachweis
(`provenance` mit `eli` und `valid_as_of`).

> Falls Port 8080 belegt ist: `MCP_HOST_PORT=8090 docker compose up --build`
> verschiebt nur den Host-Port; im Container bleibt es 8080.

## 15. Im Browser durchklicken (MCP Inspector)

Wer lieber klickt als `curl` tippt: Der offizielle **MCP Inspector** ist eine
Browser-Oberfläche für MCP-Server. Im Repository liegt eine fertige Konfiguration
(`inspector.json`), sodass ein einziger Befehl genügt:

```bash
npx -y @modelcontextprotocol/inspector --config inspector.json --server fedlex
```

Der Browser öffnet sich bereits **verbunden**. Im Tab **Tools** stehen alle Werkzeuge
mit Beschreibung und Eingabefeldern — z. B. `read_article` mit
`eli = eli/cc/1999/404` und `eid = art_1` ausprobieren.

> Die mitgelieferte Konfiguration erwartet den Server auf Port 8090
> (`MCP_HOST_PORT=8090`, siehe Kapitel 14).

## 16. An einen KI-Client anschliessen

Jede MCP-fähige Anwendung kann sich verbinden. Es braucht nur zwei Angaben:

1. **Adresse:** `http://localhost:8080/mcp` (bzw. die Produktions-URL
   `https://mcp-fedlex.ch/mcp`) — Transport «Streamable HTTP».
2. **Zugang:** der Header `Authorization: Bearer <Token>`.

Für Interessierte, die drei technischen Wege im Überblick:

| Route | Zweck |
|---|---|
| `POST /mcp` | Der empfohlene, moderne Endpunkt (MCP-Revision 2025-11-25) — mit zwei Schutz-Prüfungen *vor* jeder Verarbeitung (fremde Browser-Herkunft → 403; unbekannte Protokollversion im Header → 400). |
| `POST /rpc` | Der Legacy-Endpunkt für ältere Clients ohne Handshake — gleiche Verarbeitungskette, ohne die zwei Zusatz-Prüfungen. |
| `GET /sse` | Eröffnet einen Ereignis-Strom (Server-Sent Events) für Clients, die dieses ältere Muster nutzen. |

Der Server spricht standardmässig die MCP-Protokollrevision `2025-11-25`; ein
Alt-Client, der ausdrücklich `2024-11-05` verlangt, erhält weiterhin `2024-11-05`.
Beim Verbindungsaufbau (`initialize`) stellt sich der Server mit Namen und Version
vor und liefert dem Modell zusätzlich eine Gebrauchsanleitung («instructions»), die
u. a. den Stichtags-Parameter erklärt.

## 17. Betrieb in Produktion — der Überblick

Für alle, die wissen wollen, wie das System «richtig» läuft (Details:
[`80_DEPLOY.md`](../dev/80_DEPLOY.md)):

```
Internet → Cloudflare → Ingress (nginx) → Reader-Pods (Kubernetes)
                                              │ verschlüsselt (mTLS)
                                              ▼
                                        Redis (Kontingent-Buchhaltung)
```

- **Kubernetes (k3s) mit GitOps:** Der gewünschte Zustand liegt versioniert in Git;
  ArgoCD gleicht das Cluster automatisch damit ab.
- **Minimal-Container («distroless»):** Das Server-Image enthält keine Shell und
  keine Werkzeuge — was nicht drin ist, kann ein Angreifer nicht missbrauchen.
- **Echte Ausweise:** In Produktion nur JWT/JWKS (rotierende Schlüssel vom
  Identity-Provider) — nie das Dev-Token.
- **Verschlüsselte Kontingent-Datenbank:** Redis akzeptiert ausschliesslich TLS mit
  beidseitigen Zertifikaten (mTLS) plus Passwort; Geheimnisse liegen als
  «SealedSecrets» verschlüsselt im Git.
- **Netz dicht per Standard:** Eine Default-Deny-NetworkPolicy erlaubt nur die eine
  nötige Verbindung (Reader → Redis).
- **Beobachtbarkeit:** Health-Endpunkte (`/livez`, `/readyz`, `/startupz`) und
  Prometheus-Metriken (Aufrufe, Dauern, Lastabwürfe) — Metriken sind nur
  cluster-intern erreichbar.
- **Lastschutz:** Pro Instanz sind gleichzeitige Anfragen begrenzt; Überlast wird
  sofort und ehrlich abgewiesen (HTTP 503 mit «Retry-After») statt in unsichtbaren
  Warteschlangen zu hängen; hängende Anfragen werden nach einer harten Frist
  gekappt (HTTP 504).

---

# Teil V — Nachschlagen

## 18. Fragen und Antworten (FAQ)

### Allgemeines

**Was ist mcp-fedlex in einem Satz?**
Ein Server, der KI-Systemen zitierfähigen, stichtagsgenauen und nachprüfbaren
Zugriff auf Schweizer Bundesrecht gibt.

**Ist das ein Chatbot?**
Nein. mcp-fedlex beantwortet selbst keine Fragen in natürlicher Sprache — es ist der
Werkzeugkasten, den ein Chatbot/KI-Agent benutzt, um korrekt nachzuschlagen. Die
Anwendung, mit der Menschen sprechen, ist eine Ebene darüber (z. B. ansV).

**Ist das ein offizielles Angebot des Bundes?**
Nein. mcp-fedlex ist ein Produkt der Firma mindful.bio. Es nutzt ausschliesslich die
öffentlichen, amtlichen Daten der Fedlex-Plattform des Bundes.

**Ersetzt das eine Anwältin oder einen Anwalt?**
Nein. Es liefert belegte Normtexte und Metadaten — die juristische Beurteilung
bleibt Menschensache.

**Was kostet die Nutzung?**
Die Software ist quelloffen (Apache-2.0) und kann selbst betrieben werden. Der
Zugang zur betriebenen Instanz (mcp-fedlex.ch) ist zugangsgeschützt; dafür braucht
es ein Token von mindful.bio.

**Warum heisst es «mcp-fedlex»?**
MCP ist der offene Standard (Model Context Protocol), über den KI-Anwendungen
Werkzeuge nutzen; Fedlex ist die amtliche Publikationsplattform des Bundesrechts.
Der Name beschreibt also genau, was der Server tut: Fedlex per MCP zugänglich machen.

**In welcher Beziehung stehen mcp-fedlex, ansV und mindful.bio?**
mindful.bio ist die Firma. mcp-fedlex ist ihr Daten-Layer (dieser Server). ansV
(ansv.ch) ist die Anwendungsplattform darauf — sie nutzt den Server mit der Rolle
Navigator.

### Recht und Daten

**Woher stammen die Daten?**
Vollständig von Fedlex: Metadaten live vom öffentlichen SPARQL-Endpunkt, Gesetzestexte
als AKN-XML-Dokumente von der Fedlex-Dokumentenablage. mcp-fedlex pflegt keine eigene
Rechtsdatenbank.

**Wie aktuell sind die Antworten?**
Metadaten-Abfragen (Discovery, JoluxMetadata) gehen live an Fedlex — so aktuell wie
Fedlex selbst. Gesetzestexte werden pro Fassung einmal geholt und dann aus dem
Zwischenspeicher bedient; da eine konsolidierte Fassung unveränderlich ist, veraltet
der Zwischenspeicher inhaltlich nicht.

**Deckt es auch kantonales Recht ab? Gerichtsurteile?**
Nein, beides nicht — Fedlex publiziert Bundesrecht (Erlasse, Amtliche Sammlung,
Bundesblatt, Staatsverträge). Urteile sind kein Teil davon.

**Welche Sprachen werden unterstützt?**
Grundsätzlich die Amtssprachen Deutsch, Französisch, Italienisch, teils Englisch und
Rätoromanisch — je nachdem, was Fedlex für die konkrete Fassung publiziert. Mit
`list_expressions` prüft man vorab, welche Sprachfassungen existieren.

**Wie weit reicht die Zeitmaschine zurück?**
Für Metadaten (Fassungen, Änderungen, Daten) weit zurück. Für den maschinenlesbaren
Volltext: Fedlex stellt XML-Fassungen erst ab ca. 2021 bereit; ältere Fassungen
existieren oft nur als PDF und sind für die Volltext-Werkzeuge nicht erreichbar.

**Warum liefert ein Werkzeug eine leere Liste — ist das ein Fehler?**
Meistens nicht. Der Fedlex-Graph garantiert kaum ein Feld; fehlende Angaben sind
Daten («dazu ist nichts erfasst»), keine Fehler. Die Werkzeug-Antworten weisen an den
kritischen Stellen selbst darauf hin — z. B. beweist eine leere Änderungsliste nicht,
dass ein Erlass nie geändert wurde.

**Kann ich mit einer SR-Nummer direkt einen Text lesen?**
Nicht direkt — erst die SR-Nummer über `resolve_sr_number` zu ELI-Kandidaten
auflösen (Achtung, mehrdeutig), den richtigen wählen, dann lesen. Genau wegen dieser
Mehrdeutigkeit ist die Auflösung als Hinweis und nicht als Beleg klassiert.

**Was ist mit Anhängen und Tabellen — gehen die verloren?**
Nein. Anhänge sind über `list_components`/`list_annexes` erreichbar, Tabellen über
`extract_tables` als strukturierte Einheiten, eingebettete Formeln/Grafiken meldet
`detect_foreign_content`.

### Vertrauen und Sicherheit

**Kann die KI sich Antworten trotzdem ausdenken?**
Ein Sprachmodell kann immer Text erfinden — aber es kann keinen **Herkunftsnachweis**
erfinden: Der `provenance`-Block kommt vom Server. Eine Anwendung, die nur belegte
Aussagen akzeptiert (so arbeitet ansV), erkennt unbelegte Behauptungen sofort.

**Kann die KI das Stichtags-Datum fälschen?**
Nein. Sie kann einen Stichtag *wünschen*; was tatsächlich verwendet wurde, stempelt
der Server in `provenance.valid_as_of`. Ausserdem kann die Host-Anwendung den
Stichtag fest vorgeben — diese Vorgabe hat Vorrang vor dem Modell.

**Kann die KI ihre Rolle hochstufen oder den Mandanten wechseln?**
Nein. Rolle, Mandant und Sitzung stehen im kryptografisch geprüften Token — kein
Werkzeug-Parameter wird dafür je gelesen. Ein Token mit unbekannter Rolle wird
komplett abgewiesen.

**Was bedeutet «fail-closed»?**
Im Zweifel verweigern statt durchlassen. Beispiele: Ohne gültiges Token gibt es gar
nichts; fällt die Kontingent-Datenbank aus, gilt ein enges Notfall-Kontingent statt
unbegrenztem Zugriff; ins Audit-Log gelangen nur ausdrücklich freigegebene Felder.

**Werden meine Suchanfragen gespeichert?**
Die rohen Argumente (z. B. Suchbegriffe) und die Antwortinhalte werden **nicht**
protokolliert. Protokolliert wird pro Aufruf: pseudonymer Mandant und Sitzung, Rolle,
Werkzeugname, betroffener Erlass (ELI), Stichtag, Ergebnis-Status, Dauer (Kapitel 12).

**Sieht Mandant A, was Mandant B tut?**
Nein. Kontingent und Audit hängen am Mandanten-/Sitzungs-Paar aus dem jeweiligen
Token; es gibt keinen mandantenübergreifenden Zugriffspfad.

**Wie melde ich eine Sicherheitslücke?**
Vertraulich an **security@mindful.bio** — bitte nicht als öffentliches Issue.
Eingangsbestätigung in der Regel innert 3 Werktagen (Details: `SECURITY.md`).

### Technik (für Neugierige)

**Was genau ist MCP — und welche Version spricht der Server?**
Das Model Context Protocol, ein offener Standard für die Anbindung von Werkzeugen an
KI-Anwendungen, technisch JSON-RPC über HTTP. Der Server spricht die Revision
`2025-11-25` (Standard) und `2024-11-05` (nur wenn ein Alt-Client sie ausdrücklich
verlangt).

**Welche Protokoll-Methoden gibt es?**
`initialize` (Handshake und Versions-Aushandlung), `tools/list` (die für die Rolle
sichtbaren Werkzeuge), `tools/call` (ein Werkzeug ausführen), `ping` (Lebenszeichen)
sowie die Notification `notifications/initialized`.

**Warum drei HTTP-Routen (`/mcp`, `/rpc`, `/sse`)?**
`/mcp` ist der moderne, empfohlene Endpunkt mit zusätzlichen Schutz-Prüfungen;
`/rpc` hält ältere Clients am Leben; `/sse` bedient das ältere Ereignis-Strom-Muster.
Alle drei führen in dieselbe geprüfte Verarbeitungskette.

**Was bedeutet die Fehlermeldung `-32001 missing/invalid credential`?**
Das Bearer-Token fehlt, ist abgelaufen oder ungültig. Genau so soll sich der Server
ohne gültigen Ausweis verhalten.

**Warum bekomme ich am `/mcp`-Endpunkt 403 oder 400?**
403: Die Anfrage kam mit fremdem `Origin`-Header (Schutz gegen DNS-Rebinding aus dem
Browser); erlaubte Browser-Herkünfte pflegt der Betreiber über eine Allowlist. 400:
Der Client hat eine nicht unterstützte Protokollversion im Header angekündigt.

**Warum 503 oder 504?**
503 mit «Retry-After»: Die Instanz ist momentan voll ausgelastet und wirft Überlast
ehrlich ab — kurz warten und wiederholen. 504: Eine einzelne Anfrage hat die harte
Zeitgrenze gerissen (z. B. weil ein Upstream lahmt) und wurde gekappt.

**Warum sieht mein Client nur 13 Werkzeuge?**
Das Token trägt die Rolle Reader. Navigator sieht 39, Validator alle 40 — die
Werkzeugliste ist rollengefiltert (Kapitel 6).

**Was ist ein «Token-Bucket»?**
Das Kontingent-Modell: ein Eimer mit Guthaben, der stetig nachgefüllt wird
(Kapitel 11). Er glättet Lastspitzen, ohne normale Nutzung zu behindern.

**Warum ist der Server in Rust geschrieben?**
Speichersicherheit ohne Garbage-Collector, starke Typen für die
Sicherheits-Invarianten (z. B. ist eine geprüfte Identität ein eigener Typ, der gar
nicht anders konstruierbar ist) und vorhersagbare Performance.

**Läuft bei jedem Artikel-Abruf eine Live-Abfrage zu Fedlex?**
Nein. Beim ersten Zugriff auf eine Fassung wird deren XML einmal geholt (dazu gehört
eine kurze Metadaten-Abfrage, welche Fassung zum Stichtag gilt); danach bedient der
Zwischenspeicher. Live gehen regulär nur die Discovery- und Metadaten-Werkzeuge —
darum kosten sie im Kontingent das Fünffache.

**Wie merkt der Betrieb, dass etwas klemmt?**
Über die Health-Endpunkte (`/readyz` meldet den echten Zustand inkl.
Redis-Anbindung; ein Fedlex-Ausfall erscheint als «degraded», nimmt den Server aber
bewusst nicht vom Netz) und über Prometheus-Metriken (Aufrufe, Dauern, Lastabwürfe,
Kontingent-Notfälle).

**Kann ich den Server ohne Internetzugang betreiben?**
Nur eingeschränkt sinnvoll: Die Werkzeuge der Pools Discovery/JoluxMetadata brauchen
den Fedlex-Endpunkt live. Tests der Software selbst laufen allerdings komplett
offline (Live-Konformanz ist ein separater, ausdrücklicher Testlauf).

### Selbst betreiben und mitwirken

**Kann ich mcp-fedlex selbst betreiben?**
Ja. Lokal genügen Docker und zwei Befehle (Kapitel 14). Für Produktion gibt es
versionierte, unveränderliche Docker-Images
(`registry.mindful-server.com/mindful-bio/mcp-fedlex:v0.2.0`) und eine
Betriebsanleitung ([`80_DEPLOY.md`](../dev/80_DEPLOY.md)).

**Brauche ich Rust-Kenntnisse?**
Zum Betreiben: nein (Docker genügt). Zum Mitentwickeln: ja — Einstieg über
`CONTRIBUTING.md`.

**Woher bekomme ich ein Token?**
Lokal: das Dev-Token aus der `.env` (voller Zugriff, Rolle Validator — nur für die
Entwicklung). In Produktion: vom Identity-Provider des Betreibers; für die Instanz
mcp-fedlex.ch von mindful.bio.

**Welche Version läuft gerade?**
Der Server nennt seine Version beim `initialize`-Handshake (`serverInfo.version`);
sie entspricht dem Release-Tag. Änderungen dokumentiert das `CHANGELOG.md`.

**GitHub oder GitLab — wo lebt das Projekt?**
Die Quelle der Wahrheit (CI/CD, Releases) ist ein selbst gehostetes GitLab; GitHub
ist ein öffentlicher Spiegel. Issues/PRs auf GitHub werden gesichtet, aber im GitLab
verarbeitet.

**Wie zitiere ich das System in einer Arbeit oder Analyse?**
Mit dem versionierten Release (z. B. «mcp-fedlex v0.2.0», unveränderliches Image an
Git-Tag gebunden) plus dem Herkunftsnachweis der jeweiligen Antwort (ELI und
Stichtag). Beides zusammen macht ein Ergebnis reproduzierbar.

**Wie entsteht aus diesem Handbuch ein PDF?**
Mit einem Befehl über Pandoc — die genaue Kommandozeile steht im
Dokumentations-Index [`docs/README.md`](../README.md).

## 19. Glossar

| Begriff | Erklärung |
|---|---|
| **AKN / Akoma Ntoso** | Internationaler XML-Standard (OASIS) für juristische Dokumente. Fedlex publiziert Gesetzestexte in diesem Format; daraus lesen die Volltext-Werkzeuge. |
| **Amtliche Sammlung (AS)** | Das amtliche Publikationsorgan, in dem neues Bundesrecht verkündet wird (frz. RO). |
| **AS-/OC-Erlass** | Ein in der AS publizierter (Änderungs-)Erlass; in ELIs als `eli/oc/…` sichtbar. |
| **Audit-Log** | Das Protokoll aller Werkzeug-Aufrufe — pseudonym und ohne Inhalte (Kapitel 12). |
| **Bearer-Token** | Der «Ausweis» einer Anfrage, mitgeschickt im `Authorization`-Header. |
| **Beleg (`norm`)** | Herkunfts-Sorte «zitierfähige Aussage über einen benannten Erlass» (Kapitel 4). |
| **Bundesblatt (BBl/FGA)** | Publikationsorgan für Botschaften, Berichte und Entwürfe — die «Materialien». |
| **Claim** | Ein einzelnes, signiertes Feld in einem JWT (z. B. `role`, `tenant`). |
| **Discovery** | Werkzeug-Pool zum Auffinden von Erlassen; Ergebnisse sind Hinweise. |
| **eId** | Die stabile Adresse eines Elements **innerhalb** eines Erlasses, z. B. `art_8` oder `art_14_a`. |
| **ELI** | European Legislation Identifier — eindeutige, dauerhafte Kennung eines Erlasses, z. B. `eli/cc/1999/404`. |
| **Erlass** | Sammelbegriff für Gesetz, Verordnung, Bundesbeschluss, Staatsvertrag. |
| **Fail-closed** | Grundhaltung «im Zweifel verweigern» — bei Auth, Kontingent und Logging. |
| **Fedlex** | Die amtliche Publikationsplattform des Schweizer Bundesrechts. |
| **FRBR** | Bibliothekarisches Modell «Werk → Fassung → Datei», nach dem Fedlex seine Dokumente identifiziert. |
| **Hinweis (`hint`)** | Herkunfts-Sorte «Kandidat, kein Zitat» (Kapitel 4). |
| **Identity-Provider (IdP)** | Der Dienst, der Benutzer authentifiziert und JWTs ausstellt. |
| **JOLux** | Das Linked-Data-Vokabular/Datenmodell von Fedlex — der Metadaten-Graph über alle Erlasse. |
| **JSON-RPC** | Das einfache Anfrage-Antwort-Format, in dem MCP-Nachrichten übertragen werden. |
| **JWT / JWKS** | JSON Web Token: signiertes «digitales Ticket» mit den Claims. JWKS: der Mechanismus, über den der Server die (rotierenden) Prüfschlüssel des IdP bezieht. |
| **Konsolidierte Fassung** | Gesetzestext mit allen bis zu einem Datum eingearbeiteten Änderungen. |
| **Kubernetes / Pod** | Die Betriebsplattform in Produktion; ein Pod ist eine laufende Instanz des Servers. |
| **Mandant (Tenant)** | Die Organisation hinter einem Token; Grundlage der Trennung von Kontingent und Audit. |
| **Manifestation** | FRBR-Begriff für die konkrete Datei (z. B. das XML) einer Fassung. |
| **MCP** | Model Context Protocol — offener Standard, über den KI-Anwendungen Werkzeuge nutzen. |
| **mTLS** | Beidseitig zertifikatsgeprüfte TLS-Verbindung (hier: zwischen Reader und Redis). |
| **Pool** | Werkzeugkasten-Gruppe mit gemeinsamer Sichtbarkeits- und Kosten-Regel (Kapitel 6). |
| **Provenance** | Der serverseitig gestempelte Herkunftsnachweis jeder Antwort (`eli`, `valid_as_of`, `kind`). |
| **Quota** | Die Mengenbegrenzung pro Mandant/Sitzung (Kapitel 11). |
| **RBAC** | Role-Based Access Control — Rechte hängen an der Rolle im Token. |
| **Reader (Binary)** | Der Serverprozess von mcp-fedlex (Name des Programms; nicht zu verwechseln mit der *Rolle* Reader). |
| **Redis** | Die zentrale Datenbank für die Kontingent-Buchhaltung über alle Instanzen. |
| **Rolle** | `reader`, `navigator` oder `validator` — bestimmt sichtbare Pools und Kontingent. |
| **SPARQL** | Abfragesprache für Linked-Data-Graphen; damit werden die JOLux-Metadaten live abgefragt. |
| **SR-Nummer** | Nummer der Systematischen Rechtssammlung (z. B. SR 101 = Bundesverfassung); mehrdeutig über die Zeit. |
| **Stichtag (`as_of`)** | Das Datum, für das eine Antwort gilt; Standard: heute (Kapitel 5). |
| **Vernehmlassung** | Das formelle Konsultationsverfahren zu Gesetzesentwürfen — Entstehungskontext, kein geltendes Recht. |
| **WAF** | Web Application Firewall — Schutzfilter des Bundes vor dem Fedlex-Endpunkt. |

## 20. Weiterführende Dokumente

Dieses Handbuch erklärt das *Verständnis*. Die massgeblichen technischen Dokumente
(Entwickler- und Betriebs-Dokumentation) sind im Index
[`docs/README.md`](../README.md) verzeichnet — u. a.:

- **Konfigurations-Referenz:** [`70_CONFIG.md`](../dev/70_CONFIG.md) — alle Umgebungsvariablen.
- **Betrieb (Kubernetes):** [`80_DEPLOY.md`](../dev/80_DEPLOY.md) — Topologie, mTLS, Runbooks.
- **Identität & Rollen (technisch):** [`90_AUTH_AND_ROLES.md`](../dev/90_AUTH_AND_ROLES.md).
- **Architektur-Entscheidungen:** [`adr/`](../dev/adr) — die begründeten Weichenstellungen (ADR-001 …).
- **Capability-Lexika:** [`10_LEXICON_jolux.md`](../dev/10_LEXICON_jolux.md) und
  [`11_LEXICON_akn.md`](../dev/11_LEXICON_akn.md) — der vollständige Funktionsraum der Datenquellen.
- **Mitwirken:** `CONTRIBUTING.md` · **Sicherheit melden:** `SECURITY.md` · **Versionshistorie:** `CHANGELOG.md`.

---

*mcp-fedlex — Das Handbuch · v0.2.0 · Apache-2.0 © mindful.bio ·
Projektbeschreibung: [mcp-fedlex.ch](https://mcp-fedlex.ch) · Anwendungsplattform: [ansv.ch](https://ansv.ch)*
