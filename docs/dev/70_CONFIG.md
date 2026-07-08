# 70 — Konfigurationsreferenz (Umgebungsvariablen)

> **Was dieses Dokument ist.** Die vollständige Referenz aller Umgebungsvariablen, die der
> `mcp-reader` zur Laufzeit liest. Quelle: `crates/mcp-reader/src/main.rs`. Es gibt **keine**
> Konfigurationsdatei — die gesamte Konfiguration läuft über die Umgebung (12-Factor).

---

## 1. Netzwerk

| Variable | Pflicht | Default | Beschreibung |
| --- | --- | --- | --- |
| `BIND_ADDR` | nein | `0.0.0.0:8080` | Socket, auf dem der Reader lauscht (`HOST:PORT`). |
| `REDIS_URL` | nein | `redis://127.0.0.1:6379` | Quota-Backend (verteiltes Token-Bucket). Bei aktiviertem mTLS **muss** das Schema `rediss://` sein. |
| `MCP_REDIS_OP_TIMEOUT_MS` | nein | `2000` | Zeitgrenze pro Redis-Operation (67 §H-4). Ein *hängendes* Redis fällt so fail-closed in den pod-lokalen Fallback-Bucket, statt den Request-Pfad zu blockieren. Unparsebare Werte brechen den Start hart ab. |
| `MCP_FETCHER_CACHE_MAX_BYTES` | nein | `268435456` (256 MB) | Byte-Budget des Manifestations-Caches (Summe der XML-Größen, 67 §H-5). Eviction gewichtsbasiert (TinyLFU) statt zählbasiert. |
| `MCP_XML_MAX_BYTES` | nein | `33554432` (32 MB) | Obergrenze eines einzelnen XML-Downloads (67 §H-6). Greift beim angekündigten `Content-Length` **und** als Kappung im Stream. |

## 1a. Upstream-Zeitgrenzen (67 §H-1)

Alle ausgehenden HTTP-Aufrufe (Fedlex-SPARQL, AKN-Filestore, JWKS-Abruf) tragen zwingend
ein Connect- und ein Gesamt-Timeout — ein langsamer Upstream darf keine Tasks unbegrenzt
binden. **Unparsebare Werte brechen den Start hart ab** (kein stiller Default bei Tippfehlern).

| Variable | Pflicht | Default | Beschreibung |
| --- | --- | --- | --- |
| `MCP_UPSTREAM_CONNECT_TIMEOUT_MS` | nein | `3000` | Zeitgrenze für den TCP/TLS-Verbindungsaufbau zu Upstreams (Millisekunden). |
| `MCP_UPSTREAM_TIMEOUT_MS` | nein | `15000` | Zeitgrenze für den gesamten Upstream-Request inkl. Body (Millisekunden). Fedlex-SPARQL braucht für komplexe Queries mehrere Sekunden — großzügig, aber endlich wählen. |

## 1b'. Lastschutz der MCP-Routen (67 §H-2)

Die MCP-Routen (`/mcp`, `/rpc`, `/sse`) laufen hinter einem Timeout- und Concurrency-Layer;
Überlast wird sofort abgeworfen (HTTP 503 + `Retry-After`), gerissene Zeitgrenzen enden als
HTTP 504 mit lenkendem Hinweis. Die Health-Endpunkte liegen bewusst **ausserhalb** des
Schutzes — Probes müssen gerade unter Last antworten. **Unparsebare Werte brechen den
Start hart ab.**

| Variable | Pflicht | Default | Beschreibung |
| --- | --- | --- | --- |
| `MCP_REQUEST_TIMEOUT_MS` | nein | `30000` | Harte Zeitgrenze pro MCP-Request (Backstop **über** den Upstream-Timeouts aus §1a). |
| `MCP_MAX_CONCURRENT_REQUESTS` | nein | `256` | Maximal gleichzeitig bearbeitete MCP-Requests pro Pod; alles darüber wird sofort mit 503 abgeworfen (Fail-Fast statt unsichtbarer Warteschlange). |

## 1c. Logging (67 §O-1)

| Variable | Pflicht | Default | Beschreibung |
| --- | --- | --- | --- |
| `RUST_LOG` | nein | `info` | Log-Level/Filter (tracing-EnvFilter-Syntax, z. B. `info,audit=info`). |
| `MCP_LOG_FORMAT` | nein | `text` | `text` (lesbar, lokaler Einstieg) oder `json` (Produktion/K8s — im Deployment setzen). Die Audit-Zeile pro Tool-Call läuft als `target: "audit"` und bleibt PII-gescrubbt. |

## 1b. Protokoll-Negotiation (ADR-008 · Quelle: `src/protocol.rs`)

Der `initialize`-Handshake handelt die MCP-Protokollversion aus. Nennt der Client eine
**unterstützte** Version, wird diese ausgehandelt; nennt er **keine** (heutiger ansV-Fall), gilt die
Default-Version; nennt er eine **unbekannte/zu neue**, antwortet der Reader spec-konform mit seiner
höchsten unterstützten (kein harter Fehler). Unterstützt sind `2025-11-25` (Default, live seit
2026-06-20) und `2024-11-05` (Legacy, nur auf explizite Anfrage) — Migration abgeschlossen,
s. `docs/dev/55_MIGRATION_mcp_protocol_upgrade.md` und ADR-008.

| Variable | Pflicht | Default | Beschreibung |
| --- | --- | --- | --- |
| `MCP_PROTOCOL_DEFAULT` | nein | `2025-11-25` | Ausgehandelte Default-Version für Clients **ohne** `protocolVersion`. Wird **nur** akzeptiert, wenn der Wert in `SUPPORTED_PROTOCOL_VERSIONS` steht; sonst fail-safe auf die Kompilier-Default. Erlaubt den späteren Versionssprung als **Config-Flip ohne Redeploy** (Runbook Phase 6.2). |

## 2. Authentifizierung

Der Reader ist **fail-closed**: Ohne gültige Auth-Konfiguration ist **kein** Credential gültig und
jeder Aufruf endet mit `-32001 missing/invalid credential`. Die Auswahl erfolgt in dieser
**Reihenfolge** (erste passende gewinnt):

1. `MCP_JWT_JWKS_URL` → JWKS-Modus (rotierende Schlüssel vom IdP)
2. `MCP_JWT_HS256_SECRET` → HS256 mit statischem Secret
3. `MCP_JWT_RS256_PUBKEY_FILE` → RS256 mit PEM-Public-Key
4. `MCP_DEV_TOKEN` → statisches Dev-Token (Rolle `Validator`, Mandant `dev`)

| Variable | Pflicht | Default | Beschreibung |
| --- | --- | --- | --- |
| `MCP_JWT_ISSUER` | bedingt | — | **Pflicht in jedem JWT-Modus.** Erwarteter `iss`-Claim. |
| `MCP_JWT_AUDIENCE` | nein | — | Optionaler `aud`-Claim. Wenn gesetzt, wird er geprüft. |
| `MCP_JWT_JWKS_URL` | nein | — | JWKS-Endpunkt. Aktiviert den JWKS-Modus. |
| `MCP_JWT_JWKS_REFRESH_SECS` | nein | `300` | Abrufintervall des JWKS in Sekunden. |
| `MCP_JWT_HS256_SECRET` | nein | — | Symmetrisches Secret (HS256). |
| `MCP_JWT_RS256_PUBKEY_FILE` | nein | — | Pfad zu einer PEM-Datei mit RSA-Public-Key (RS256). |
| `MCP_DEV_TOKEN` | nein | — | Statisches Bearer-Token für die lokale Entwicklung. **Nicht in Produktion.** |

> Rollenmodell und Claims-Schema (`role`/`tenant`/`session`): siehe `docs/dev/90_AUTH_AND_ROLES.md`.

## 3. Redis-mTLS (optional, Produktion · ADR-005)

Diese drei Variablen müssen **gemeinsam** gesetzt sein (alle drei oder keine). Sind sie gesetzt,
verbindet sich der Reader gegenseitig authentifiziert über `rediss://`; `REDIS_URL` **muss** dann
mit `rediss://` beginnen, sonst bricht der Start bewusst ab (kein stillschweigender Klartext).

| Variable | Pflicht | Default | Beschreibung |
| --- | --- | --- | --- |
| `MCP_REDIS_TLS_CA_FILE` | bedingt | — | CA-Zertifikat (PEM), das das Redis-Server-Zert beglaubigt. |
| `MCP_REDIS_TLS_CERT_FILE` | bedingt | — | Client-Zertifikat (PEM) des Readers. |
| `MCP_REDIS_TLS_KEY_FILE` | bedingt | — | Privater Schlüssel (PEM) zum Client-Zert. |

Ohne dieses Material bleibt die Redis-Verbindung Klartext — im Cluster abgesichert durch die
Default-Deny-NetworkPolicy, lokal durch das interne compose-Netz.

## 4. Health-Endpunkte (keine Konfiguration, zur Referenz)

| Pfad | Zweck |
| --- | --- |
| `GET /livez` | Liveness — anschlagsfrei, prüft keinen Upstream. |
| `GET /readyz` | Readiness — **kritisch**: Quota-Redis; **informativ**: Fedlex-SPARQL (Ausfall = `degraded` im Body, nicht unready; 67 §H-8). |
| `GET /startupz` | Startup — wird grün, sobald die Komposition steht (kein Warmup; Cache füllt lazy, 67 §H-5/W-1). |
| `GET /metrics` | Prometheus-Metriken (67 §O-2): `mcp_tool_calls_total{tool,outcome}`, `mcp_tool_call_duration_ms{tool}`, `mcp_requests_shed_total`, `mcp_requests_timed_out_total`, `mcp_quota_fallback_total`, `mcp_upstream_short_circuit_total{upstream}`. **Am Ingress nicht öffentlich routen** — nur in-cluster scrapen. |

## 5. Minimalbeispiele

**Lokal (Dev-Token, Klartext-Redis):**
```bash
BIND_ADDR=0.0.0.0:8080
REDIS_URL=redis://redis:6379
MCP_DEV_TOKEN=dev-secret-change-me
```

**Produktion (JWKS + Redis-mTLS):**
```bash
BIND_ADDR=0.0.0.0:8080
REDIS_URL=rediss://mcp-reader-redis:6379
MCP_JWT_ISSUER=https://idp.example.com/
MCP_JWT_AUDIENCE=mcp-fedlex
MCP_JWT_JWKS_URL=https://idp.example.com/.well-known/jwks.json
MCP_REDIS_TLS_CA_FILE=/etc/redis-tls/ca.crt
MCP_REDIS_TLS_CERT_FILE=/etc/redis-tls/client.crt
MCP_REDIS_TLS_KEY_FILE=/etc/redis-tls/client.key
```
