# 66 — Web-Review-Findings (mcp-fedlex-web)

> **Was dieses Dokument ist.** Vollständiges Review-Protokoll der Doku-/Marketing-Site
> **`mcp-fedlex-web`** (Zola, mcp-fedlex.ch), festgehalten hier im Server-Repo, damit Server-
> und Site-Doku an einem Ort zusammenlaufen. Es ist der Rohstoff für eine priorisierte
> ToDo-Liste (§ ToDo-Basis am Ende). Abgrenzung: **[65_REVIEW_FINDINGS.md](65_REVIEW_FINDINGS.md)**
> registriert Befunde am *Server*; dieses Dokument die Befunde an der *Website*.
> IDs hier: `WF-<n>`. **Status-Werte:** 🔴 offen · 🟡 in Arbeit · 🟢 erledigt · ⚪ verworfen.
>
> **Stand: 2026-07-02.** Methode: alle 36 Repo-Dateien gelesen; `zola build` + `zola check`
> lokal ausgeführt (grün, 20 Seiten); generiertes HTML in `public/` inspiziert; Live-Site
> (Header, `/mcp`-Routing, 404-Verhalten) per `curl` geprüft; Content gegen die Ground Truth
> des Servers abgeglichen (`mcp-fedlex` v0.2.0, MCP `2025-11-25`, Ingress in
> `k3-infra/manifests/workloads/mcp-fedlex/ingress.yaml`).
>
> **Abarbeitung 2026-07-02:** 21 von 23 Findings umgesetzt (Commits im Web-Repo `6a03f8f…a2b6d3f`,
> im Server-Repo `7795698`/`83ae577`), Ergebnis im Container end-to-end verifiziert (404,
> Security-Header, gzip, hreflang). **Nicht gepusht** — Push auf `main` deployt via CI/ArgoCD.
> Offen: WF-14 (Slug-Entscheidung), WF-22 (juristische Prüfung), E-1 (leeres Verzeichnis).

---

## Gesamtbild

Die Site ist in **gutem Zustand**: Der fachliche Content ist stark und aktuell, die
Übersetzungen sind vollständig und strukturell synchron, das CSS ist handwerklich sauber
(Design-Tokens, Dark-Mode, `prefers-reduced-motion`, Tap-Targets ≥ 44 px), Deploy läuft
(zuletzt 2026-07-01). Die Befunde liegen fast ausschließlich in **drei Zonen**:
(A) Meta-Doku driftet (CLAUDE.md/README widersprechen Repo und Ingress),
(B) Serving-Härtung fehlt (404, Security-Header, CI-Linkcheck),
(C) SEO/i18n-Basics fehlen (hreflang, canonical, Sprachumschalter, Descriptions).

### Verifiziert in Ordnung (nicht anfassen)

| Geprüft | Beleg |
|---|---|
| **Tool-Katalog 100 % code-deckungsgleich**: alle 25 Tool-Namen, 4 Pools (11/3/10/1), Rollen Reader ⊆ Navigator ⊆ Validator | `content/werkzeuge.md` ↔ `mcp-reader` `main.rs`/`tools.rs`/`discovery.rs`/`metadata.rs` |
| Zahlenwerk Funktionsraum konsistent: 47 Primitive = 24 projiziert + 23 reserviert; 24 + 1 Composite = 25 | `werkzeuge.md:11-19`, in allen 5 Sprachen identisch |
| Protokoll-Angabe `2025-11-25`, Endpoints `/mcp` `/rpc` `/sse`, Dev-Token, JWT-Claims | `schnellstart.md` ↔ `protocol.rs:31/39`, `transport.rs`, `docs/90` |
| Übersetzungen strukturell synchron (5 × 5 Dateien, identische Zeilenzahl), interne Links korrekt lokalisiert (`/fr/werkzeuge/` …) | `wc -l content/*` ; grep der Link-Pfade |
| `zola build` + `zola check` grün; `robots.txt` + `sitemap.xml` (alle 25 URLs) werden generiert | lokaler Build 2026-07-02 |
| Live-Routing `/mcp` → Reader funktioniert (405 auf GET = Reader antwortet, nicht die Site) | `curl -I https://mcp-fedlex.ch/mcp` ; `ingress.yaml:37` |
| A11y-Grundlagen: `:focus-visible`-Ringe, Fokus auf Checkbox-Burger, `aria-label`s, `lang`-Attribut korrekt pro Sprache | `style.css:119`, `base.html` |

---

## A — Meta-Doku-Drift (CLAUDE.md / README)

### WF-1 — CLAUDE.md ist untracked (nie committet)
- **Status:** 🟢 erledigt (2026-07-02, `f871de5`)
- **Beleg:** `git status` ⇒ `?? CLAUDE.md`; letzter Commit `3edf6f3` enthält sie nicht.
- **Wirkung:** Die Betriebsanleitung der Site existiert nur auf einer Maschine; CI/andere
  Checkouts/Agenten sehen sie nicht. Änderungen sind nicht reviewbar.
- **Behandlung:** Nach Korrektur der Faktenfehler (WF-2) committen — in **derselben** Änderung
  wie die README-Angleichung (WF-3), damit die Truth-Hierarchie (WF-4) ab dem ersten Commit stimmt.

### WF-2 — CLAUDE.md-Faktenfehler: Seitenzahl und Routing veraltet
- **Status:** 🟢 erledigt (2026-07-02, `f871de5`)
- **Beleg:** `CLAUDE.md:31` behauptet „**20 Markdown-Dateien = 4 Seiten × 5 Sprachen**" — real
  sind es **25 Dateien = 5 Seiten × 5 Sprachen** (Impressum kam mit `3edf6f3` dazu).
  `CLAUDE.md:10-12` nennt als API-Pfade nur `/rpc, /sse, /livez, /readyz` — der Ingress routet
  zusätzlich **`/mcp`** (`ingress.yaml:37`), das zugleich der auf der Site empfohlene
  Hauptendpoint ist (`schnellstart.md:20,59`).
- **Wirkung:** Wer der CLAUDE.md folgt, hält `https://mcp-fedlex.ch/mcp` für einen Pfad der
  statischen Site — genau die Drift-Sorte, die `RF-1` serverseitig schon einmal korrigiert hat.
- **Behandlung:** Beide Stellen korrigieren (5 Seiten inkl. Impressum; `/mcp` in die
  Routing-Aufzählung). Kommentarkopf von `config.toml:3-4` gleich mitziehen (nennt Impressum
  ebenfalls nicht).

### WF-3 — README.md veraltet: „viersprachig", `/mcp` fehlt, toter Versions-Verweis
- **Status:** 🟢 erledigt (2026-07-02, `f871de5`)
- **Beleg:** `README.md:7` „viersprachig de/fr/it/rm" (real fünf, `en` seit `0683b4a`);
  `README.md:20` Routing ohne `/mcp`; `README.md:24` behauptet „welche Server-Version
  beschrieben ist, steht in `content/architektur.md`" — dort steht **keine** Versionsangabe
  (grep über `content/` ⇒ kein Treffer auf eine Server-Version).
- **Wirkung:** Drei falsche Fakten in 25 Zeilen README; der Versions-Verweis führt ins Leere.
- **Behandlung:** README korrigieren; der Versions-Verweis wird erst mit WF-21 wieder wahr
  (Versionsangabe in den Content einbauen) — beides zusammen erledigen.

### WF-4 — Truth-Hierarchie invertiert und sprachlich mehrdeutig
- **Status:** 🟢 erledigt (2026-07-02, `f871de5`)
- **Beleg:** `CLAUDE.md:8-9`: „README.md ist die ausführliche Quelle der Wahrheit. Diese Datei
  ist die destillierte Kurzfassung — bei Konflikt gewinnt **sie**." — Referenz von „sie" ist
  ambig; real ist die README (25 Zeilen, 3 Fehler, s. WF-3) *dünner und falscher* als die
  CLAUDE.md (55 Zeilen, detailliert). Vorbild `mcp-fedlex/CLAUDE.md` formuliert eindeutig
  („bei Konflikt gewinnen jene").
- **Wirkung:** Die deklarierte Hierarchie beschreibt das Gegenteil des Ist-Zustands; bei
  Konflikt würde man der falscheren Quelle glauben.
- **Behandlung:** Entscheiden: entweder README zur echten ausführlichen Quelle ausbauen
  (Struktur/Deploy/Content-Pflege dorthin) oder die Hierarchie umdrehen (CLAUDE.md maßgeblich,
  README = öffentliches Schaufenster). Formulierung eindeutig machen.

---

## B — Serving & Build-Härtung (nginx / Docker / CI)

### WF-5 — 404-Erlebnis: ungestylte Zola-Default-Seite, von nginx nicht mal ausgeliefert
- **Status:** 🟢 erledigt (2026-07-02, `07475bd`)
- **Beleg:** `public/404.html` = Zolas eingebautes 3-Zeilen-Template („404 Not Found", ohne
  CSS/Nav). nginx kennt es nicht einmal: `nginx.conf:17` `try_files … =404` ⇒ live erscheint
  die nginx-Standardseite **inkl. Versionsbanner `nginx/1.31.2`**
  (`curl https://mcp-fedlex.ch/gibtsnicht`).
- **Wirkung:** Jeder Tippfehler in einer URL endet auf einer gebrandeten Fremdseite —
  Vertrauensverlust auf einer Site, deren Kernbotschaft „Nachvollziehbarkeit" ist; zudem
  Version-Disclosure.
- **Behandlung:** (1) `templates/404.html` gestalten (erbt `base.html`, Links auf Start/
  Werkzeuge/Schnellstart, mehrsprachiger Kurztext); (2) `nginx.conf`: `error_page 404
  /404.html;` + `server_tokens off;`.

### WF-6 — Keine Security-Header am Origin
- **Status:** 🟢 erledigt (2026-07-02, `0e210d1`)
- **Beleg:** `curl -sI https://mcp-fedlex.ch/` ⇒ weder `X-Content-Type-Options` noch
  `Referrer-Policy`, `Content-Security-Policy`, `Permissions-Policy`, `X-Frame-Options`/
  `frame-ancestors`; auch Cloudflare ergänzt nichts. `nginx.conf` setzt keinerlei Header.
- **Wirkung:** Für eine statische Site begrenztes Risiko, aber: die Site wirbt für einen
  fail-closed-Security-Server — der eigene Auftritt sollte die Basics vorleben. CSP scheitert
  aktuell am Inline-Copy-Button-Script (`base.html:53-115`).
- **Behandlung:** Copy-Button-JS nach `static/copy.js` auslagern (wird dank
  `cachebust=true`-Muster sauber gecacht), dann in `nginx.conf`: `X-Content-Type-Options:
  nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy`
  (Minimalset), `frame-ancestors 'none'` via CSP, `default-src 'self'`. HSTS bewusst am
  Cloudflare-Edge entscheiden, nicht am Origin.

### WF-7 — Kein gzip am Origin
- **Status:** 🟢 erledigt (2026-07-02, `13e2f8f`)
- **Beleg:** `nginx.conf` ohne `gzip on`; nginx-Default komprimiert nur nichts bzw. nur HTML.
- **Wirkung:** `style.css` (~14 KB) und HTML gehen unkomprimiert Origin→Cloudflare; klein,
  aber gratis zu holen.
- **Behandlung:** `gzip on; gzip_types text/css application/javascript image/svg+xml
  application/xml text/plain; gzip_min_length 512;`

### WF-8 — Kein `.dockerignore` (Schwester-Repo hat eins)
- **Status:** 🟢 erledigt (2026-07-02, `b61cf98`)
- **Beleg:** Repo-Root ohne `.dockerignore`; `Dockerfile:4` `COPY . .` zieht `.git/`, lokales
  `public/`, `README`, `CLAUDE.md` in den Kaniko-Build-Kontext. `mcp-fedlex` hat ein
  `.dockerignore`.
- **Wirkung:** Größerer Build-Kontext, unnötige Cache-Invalidierung; ein lokal gebautes
  `public/` wandert mit in den Builder (Zola überschreibt es, aber der Kontext-Upload bleibt).
- **Behandlung:** `.dockerignore` mit `public/`, `.git/`, `*.md`-Doku (außer `content/`!),
  `.gitlab-ci.yml`. Vorsicht: `content/**/*.md` muss selbstverständlich rein.

### WF-9 — Runtime-Image ungepinnt (`:alpine` = moving target)
- **Status:** 🟢 erledigt (2026-07-02, `652cba3`)
- **Beleg:** `Dockerfile:8` `FROM nginxinc/nginx-unprivileged:alpine` — Build-Stage ist exakt
  gepinnt (`zola:v0.22.1`), Runtime nicht; live läuft dadurch Mainline `nginx/1.31.2`.
- **Wirkung:** Nicht reproduzierbare Images; ein Upstream-Bump ändert das Prod-Verhalten ohne
  Commit im Repo.
- **Behandlung:** Auf Minor pinnen (z. B. `nginx-unprivileged:1.28-alpine`, Stable-Linie) oder
  per Digest; Update-Pfad in CLAUDE.md notieren.

### WF-10 — `zola check` läuft nirgends automatisch
- **Status:** 🟢 erledigt (2026-07-02, `2120301`)
- **Beleg:** `.gitlab-ci.yml`: der `check`-Job führt nur den Kaniko-Build aus (`zola build`
  im Dockerfile); `zola check` ist laut `CLAUDE.md:41` „lokal vor Commit empfohlen" — reine
  Disziplin. Zusätzlich läuft der `check`-Job **nur auf Nicht-main-Branches**; Direkt-Pushes
  auf `main` validieren nichts vor dem Image-Push.
- **Wirkung:** Tote interne/externe Links erreichen unbemerkt Produktion; genau die Lücke,
  die die Site-Konvention „Content mit dem Server synchron halten" verschärft.
- **Behandlung:** Im `Dockerfile` vor dem Build eine Check-Zeile ergänzen:
  `RUN ["zola", "check", "--skip-external-links"]` (Flag in v0.22.1 verifiziert; externe Links
  bewusst skippen — kein Netz-Flake im Kaniko-Build). Damit prüft jeder CI-Build (auch main)
  die Links mit.

---

## C — SEO & i18n-Mechanik

### WF-11 — Kein `hreflang`, kein `canonical`
- **Status:** 🟢 erledigt (2026-07-02, `3c55b0a`)
- **Beleg:** `base.html` `<head>` enthält weder `<link rel="alternate" hreflang="…">` noch
  `<link rel="canonical">`; Sitemap ohne `xhtml:link`-Alternates (Build-Inspektion).
- **Wirkung:** Suchmaschinen erkennen die 5 Sprachvarianten nicht als Übersetzungs-Cluster;
  Duplicate-Content-Risiko (zumal `try_files $uri $uri/` beide Slash-Varianten mit 200
  beantworten kann). Für eine fünfsprachige Site *das* SEO-Fundament.
- **Behandlung:** In `base.html` aus `page.translations`/`section.translations` (Zola stellt
  `lang` + `permalink` bereit) die `hreflang`-Reihe inkl. `x-default` (DE) generieren;
  `canonical` = `current_url`.

### WF-12 — Sprachumschalter wirft den Leser auf die Startseite zurück
- **Status:** 🟢 erledigt (2026-07-02, `3c55b0a`)
- **Beleg:** `base.html:31-35`: die Links DE/FR/IT/RM/EN zeigen hart auf `/`, `/fr/`, `/it/`,
  `/rm/`, `/en/` — unabhängig von der aktuellen Seite.
- **Wirkung:** Wer `/werkzeuge/` liest und FR wählt, landet auf `/fr/` statt
  `/fr/werkzeuge/` — Kontextverlust auf jeder Unterseite, in 4 von 5 Sprachen der erste
  Eindruck.
- **Behandlung:** Gleiches Fundament wie WF-11: Umschalter aus `translations`-Daten bauen,
  Fallback Startseite, `rel="alternate"`-Semantik gratis. Ein Template-Edit, alle Seiten
  profitieren.

### WF-13 — Alle Seiten einer Sprache teilen dieselbe Meta-Description
- **Status:** 🟢 erledigt (2026-07-02, `0c33b55`)
- **Beleg:** `base.html:7` nutzt immer `config.description`; Frontmatter der Seiten hat kein
  `description`-Feld. Build-Inspektion: `fr/werkzeuge/index.html` trägt die generische
  Site-Description.
- **Wirkung:** Suchtreffer-Snippets sind für Werkzeuge/Schnellstart/Architektur identisch —
  verschenkte Klickrate auf den inhaltsstärksten Seiten.
- **Behandlung:** `description = "…"` ins Frontmatter aller 25 Content-Dateien;
  `{% if page.description %}`-Weiche in `base.html`.

### WF-14 — Deutsche Slugs in allen Sprachen (`/en/werkzeuge/`)
- **Status:** 🔴 offen — **Nutzerentscheidung nötig** (URL-Wechsel bricht bestehende Links; Rest s. Behandlung)
- **Beleg:** URLs aller Sprachen nutzen die deutschen Datei-Stämme: `/en/werkzeuge/`,
  `/fr/schnellstart/` (Sitemap).
- **Wirkung:** Für FR/IT/EN-Suchende sind die URLs semantisch leer („werkzeuge" rankt nicht
  für „outils"); auch UX-seitig irritierend. Gegenposition: stabile, sprachneutrale URLs sind
  pflegeleichter — legitime Entscheidung, aber sie sollte *dokumentiert* fallen.
- **Behandlung:** Entscheiden. Falls lokalisieren: `slug = "outils"` etc. im Frontmatter der
  Übersetzungen; Nav-Links in `base.html` müssen dann aus Section-Daten statt hartkodiert
  kommen (berührt die WF-12-Lösung). Falls nicht: in CLAUDE.md als bewusste Entscheidung
  festhalten.

### WF-15 — Keine Open-Graph-/Twitter-Card-Tags
- **Status:** 🟢 erledigt (2026-07-02, `3469730`)
- **Beleg:** `base.html` `<head>` ohne `og:title`, `og:description`, `og:locale`, `og:image`,
  `twitter:card`.
- **Wirkung:** Links auf LinkedIn/X/Slack/Teams rendern ohne Vorschau — für eine Site, deren
  Zielgruppe (Legal-Tech, Kanzleien, Bundesnähe) Links primär teilt, ein realer Reichweiten-
  Verlust. Ein `og:image` (Wortmarke + Tagline) fehlt als Asset.
- **Behandlung:** Meta-Block in `base.html` (aus denselben Quellen wie WF-13); ein statisches
  `og-image.png` (1200×630) je Sprache oder sprachneutral.

---

## D — Content, Typografie, A11y-Feinschliff

### WF-16 — Deklarierte Fonts werden nie geladen (Inter / IBM Plex Mono)
- **Status:** ⚪ bewusst belassen (2026-07-02, `dada601`) — mindful-bio-web lädt Inter ebenfalls nicht; Self-Hosting nur hier würde die Sites optisch auseinandertreiben. In CLAUDE.md dokumentiert.
- **Beleg:** `style.css:28-30` deklariert `"Inter Display", "Inter"` und `"IBM Plex Mono"`;
  kein `@font-face`, kein Font-Link in `base.html`, keine Font-Dateien in `static/` —
  `nginx.conf:20` cached sogar `woff2`, die es nicht gibt.
- **Wirkung:** Die Site rendert faktisch überall im System-Font. Entweder ist die
  Font-Deklaration totes Wunschdenken oder das Brand-Erscheinungsbild (geerbt von
  `mindful-bio-web`, s. `style.css:3`) kommt nie an.
- **Behandlung:** Entscheiden: (a) Inter + IBM Plex Mono als `woff2` self-hosten
  (`static/fonts/`, `@font-face` mit `font-display: swap`, `preload` für Body-Font) — dann
  stimmt auch die Design-Token-Vererbung; oder (b) Font-Stack ehrlich auf `system-ui`
  kürzen. Kein Google-Fonts-CDN (Datenschutz, s. WF-22).

### WF-17 — Kein Syntax-Highlighting auf einer Entwickler-Doku-Site
- **Status:** 🟢 erledigt (2026-07-02, `1761e9c`) — Zola 0.22 highlightet via Giallo: `highlighting = { style = "class", theme = … }`; Inline-Modus wäre an der CSP gescheitert
- **Beleg:** `config.toml` ohne `[markdown] highlight_code = true` (Zola-Default: aus);
  Codeblöcke in Schnellstart/Werkzeuge rendern monochrom.
- **Wirkung:** Die wichtigsten Seiten (curl-/JSON-Beispiele) verschenken Lesbarkeit; der
  Copy-Button zeigt, dass Code-UX dem Projekt wichtig ist — Highlighting fehlt als Gegenstück.
- **Behandlung:** `highlight_code = true` mit `highlight_theme = "css"` und eigenen
  Token-Farben in `style.css` (Light + Dark aus den bestehenden Design-Tokens — vorgefertigte
  Zola-Themes brechen den Dark-Mode).

### WF-18 — Footer-Texte in 4 von 5 Sprachen falschsprachig
- **Status:** 🟢 erledigt (2026-07-02, `5289b18`)
- **Beleg:** `base.html:46-48`: „Quellcode" und „Live ausprobieren auf ansv.ch" sind
  hartkodiert deutsch; `nav_imprint` daneben ist korrekt übersetzt.
- **Wirkung:** Auf jeder EN/FR/IT/RM-Seite steht deutscher Text im Footer — gerade für die
  rätoromanische Vollständigkeit (USP der Site) ein Schönheitsfehler.
- **Behandlung:** Zwei Translation-Keys (`footer_source`, `footer_try_live`) in `config.toml`
  (× 5 Sprachen), Footer auf `trans()` umstellen.

### WF-19 — Tote Datei-Referenzen im öffentlichen Content
- **Status:** 🟢 erledigt (2026-07-02, `30b18b4`)
- **Beleg:** `werkzeuge.md:11,19` nennt `10_LEXICON_jolux.md`, `11_LEXICON_akn.md`,
  `lexicon_projection.rs`; `schnellstart.md:65` nennt `docs/90_AUTH_AND_ROLES.md`,
  `docs/80_DEPLOY.md` — alles als Backtick-Text ohne Link.
- **Wirkung:** Website-Besucher können den zentralen Vertrauensanker („jede Reservierung
  begründet, CI-abgesichert") nicht nachprüfen — dabei liegen die Dateien öffentlich auf
  GitHub.
- **Behandlung:** Auf `https://github.com/mindful-bio/mcp-fedlex/blob/main/docs/…` verlinken
  (× 5 Sprachen). Konvention festhalten: Repo-Dateien im Web-Content immer als Link.

### WF-20 — `/sse` wird als gleichwertige Alternative präsentiert, ist aber Legacy
- **Status:** 🟢 erledigt (2026-07-02, `d124cee`)
- **Beleg:** `schnellstart.md:59` „alternativ eröffnet `GET /sse` einen SSE-Strom" —
  Server-Doku stuft SSE als Alt-Transport ohne Erhaltungsziel ein
  (`mcp-fedlex/docs/55_MIGRATION_mcp_protocol_upgrade.md` §0); Empfehlung ist Streamable
  HTTP `POST /mcp`.
- **Wirkung:** Neue Integratoren könnten auf den Transport bauen, der als erstes fallen wird.
- **Behandlung:** `/mcp` als den Weg formulieren, `/sse` (und `/rpc`) als Legacy für
  Alt-Clients kennzeichnen (× 5 Sprachen) — deckungsgleich mit dem Server-README.

### WF-21 — Beschriebene Server-Version steht nirgends im Content
- **Status:** 🟢 erledigt (2026-07-02, `6a03f8f`)
- **Beleg:** grep über `content/` ⇒ keine Versions-/Stand-Angabe; Web-README behauptet sie
  (s. WF-3). Server ist inzwischen als **v0.2.0** getaggt.
- **Wirkung:** Die Site verspricht Stichtagsgenauigkeit als Produkt-USP, datiert aber die
  eigene Beschreibung nicht — Leser können nicht erkennen, ob „25 Werkzeuge" noch stimmt.
- **Behandlung:** Ein Satz in `architektur.md` (× 5): „Beschrieben ist mcp-fedlex **v0.2.0**
  (MCP `2025-11-25`)." In die Release-Checkliste des Servers aufnehmen: Website-Versionssatz
  mitziehen (Konvention existiert schon in `CLAUDE.md:40`, ihr fehlt nur der konkrete Anker).

### WF-22 — Impressum ohne Datenschutzerklärung
- **Status:** 🟡 Entwurf committet (`5afcb03`) — **vor dem Push juristisch gegenlesen lassen** (revDSG-Formulierungen, DPF-Verweis, Löschfristen)
- **Beleg:** `content/impressum.md` = nur Firmenangaben. Live sendet Cloudflare
  NEL/`report-to`-Telemetrie (Response-Header, `curl -sI`); Betreiberin ist eine GmbH, die
  über die Site einen gehosteten Dienst anbietet („Zugangsdaten auf Anfrage",
  `schnellstart.md:63`).
- **Wirkung:** Nach revDSG (Art. 19 ff., Informationspflicht) ist bei Bearbeitung von
  Personendaten (Cloudflare-Logs/NEL, E-Mail-Kontakt, künftig Kunden-Onboarding) eine
  Datenschutzerklärung der Normalfall. Keine Rechtsberatung — aber für ein Legal-Tech-Produkt
  ist die Lücke exponiert.
- **Behandlung:** Kurze Datenschutz-Seite (× 5 Sprachen: Hosting/Cloudflare, Server-Logs,
  Kontakt, keine Cookies/kein Tracking — die Site ist ja vorbildlich trackerfrei); juristisch
  gegenlesen lassen.

### WF-23 — Sammelposten Kleinkram
- **Status:** 🟢 erledigt (2026-07-02, `a2b6d3f`) — Tabellen jetzt per JS-Wrapper (site.js) statt display:block
- **Belege & Behandlung:**
  1. `.note`-CSS-Klasse ungenutzt (`style.css:248-251`, grep ohne Treffer im Content) → löschen.
  2. `table { display: block }` (`style.css:480-486`) zerstört in manchen Browsern die
     Tabellen-Semantik für Screenreader → Markdown-Tabellen in einen Scroll-Wrapper legen
     (kleines JS im ohnehin geplanten `copy.js`, s. WF-6) oder `role="table"`-freundliche
     Overflow-Lösung dokumentieren.
  3. Kein Skip-Link („Zum Inhalt springen") vor der Nav (`base.html:12`) → einzeiliger
     `<a class="skip-link" href="#main">` + `id="main"`.
  4. Commit-Konvention: `CLAUDE.md:47` fordert Conventional Commits; 7 von 9 Commits der
     Historie folgen dem nicht (`git log --oneline`) → Konvention gilt ab jetzt, in CLAUDE.md
     ehrlich als „ab 2026-07" markieren oder streichen.
  5. `config.toml:3-4` Kommentar nennt Impressum nicht (s. WF-2).

---

## E — Randnotizen außerhalb des Web-Repos

| # | Befund | Beleg | Vorschlag |
|---|---|---|---|
| E-1 🔴 | `mcp-fedlex-web-app/` ist ein **leeres Verzeichnis** (angelegt 2026-06-29) neben `mcp-fedlex-web/` — Namenskollisions-Gefahr für Menschen und Agenten | `ls -la mindful.bio/mcp-fedlex-web-app` | Zweck klären; entweder README-Stub mit Abgrenzung („geplante interaktive Web-App, nicht die Doku-Site") oder löschen |
| E-2 🟢 (`7795698`) | `RF-2` in [65_REVIEW_FINDINGS.md](65_REVIEW_FINDINGS.md) stand 🔴 „v0.2.0 taggen", aber Tag `v0.2.0` **existiert** (Commit `cbd919b`) | `git tag` im Server-Repo | RF-2 auf 🟢 setzen, Erledigungsdatum nachtragen |
| E-3 🟢 (`83ae577`) | Server-Doku uneinheitlich bei Pool-Zahl: Code deklariert 6 `ToolPool`-Varianten (2 leer), `docs/90` listet 6, README/CLAUDE.md/Website sagen „vier (aktive) Pools" | `tool.rs:20-45` vs. `docs/90:38-45` | In `docs/90` die zwei leeren Pools explizit als „deklariert, unbestückt" markieren — Website bleibt bei „vier Pools" korrekt |

---

## ToDo-Basis (Priorisierungsvorschlag)

**P1 — Vertrauen & Korrektheit (zuerst, alles kleine Diffs):**
WF-1/2/3/4 (Meta-Doku in einem Commit heilen) · WF-5 (404 + `server_tokens`) ·
WF-21 (Versionssatz) · WF-20 (SSE-Legacy-Hinweis) · E-2 (Register nachführen)

**P2 — Reichweite & Robustheit:**
WF-11 + WF-12 (ein Template-Umbau, gemeinsame Datenbasis) · WF-13 · WF-15 ·
WF-10 (CI-Linkcheck) · WF-6 (Header, nach JS-Auslagerung) · WF-8 · WF-9 ·
WF-19 (Links) · WF-22 (Datenschutz klären) · WF-16 (Font-Entscheidung)

**P3 — Kür:**
WF-17 (Highlighting) · WF-14 (Slug-Entscheidung) · WF-7 (gzip) · WF-18 (Footer-Keys) ·
WF-23 (Sammelposten) · E-1/E-3

> **Pflegehinweis.** Erledigte WF-Einträge hier auf 🟢 setzen (Datum + Commit), nicht löschen —
> gleiche Disziplin wie in 65. Wandert ein Punkt in eine echte Planung, Verweis eintragen.
