# Swiss-Legal auf ECS — Deploy-Runbook (506)

Dieses Verzeichnis gehört **506** und existiert nur in unserem Fork
(`506-Data-Performance/mcp-fedlex`) — upstream (`mindful-bio/mcp-fedlex`)
kennt es nicht, darum gibt es beim Fork-Sync nie Konflikte.

## Architektur in einem Satz

EIN Fargate-Service (`swiss-legal` im `companygpt-cluster`) für die gesamte
CompanyGPT-Flotte, erreichbar über den vorhandenen internen ALB
(`ecs-cgpt-services-internal-lb`, Listener **:53000**), Mandanten-Trennung
per JWT (`tenant`-Claim) — der Server hält keine Daten, alles kommt live von
der amtlichen Fedlex-Plattform.

```
Kundenbox (fedlex-Skill) ──https://swiss-legal.506.ai:53000/mcp──►  interner ALB
                                                                        │ /readyz-Health
                                                                  Fargate-Task
                                                                    ├─ reader :8080
                                                                    └─ redis (localhost)
                                                                        │ live
                                                                    Fedlex (Bund)
```

## Einmalig eingerichtet (03.09.2026)

| Baustein | Wert |
|---|---|
| ECR | `506data/mcp-fedlex` — Tags IMMUTABLE, Lifecycle: letzte 10 |
| Secret | `SWISS_LEGAL_HS256_SECRET` (Secrets Manager) — **identisch mit der alten EC2-Box**, damit ausgestellte Kunden-JWTs gültig bleiben. Nie rotieren ohne alle Tokens neu auszustellen! |
| Task-Familie | `swiss-legal` (Fargate 0.25 vCPU / 512 MB, reader + redis-Sidecar) |
| Logs | CloudWatch `/ecs/swiss-legal` (JSON; eine Audit-Zeile pro Tool-Call) |
| Health | ALB-Check auf `/readyz` — **kein** Container-Healthcheck (distroless: kein Shell) |
| DNS | `swiss-legal.506.ai` → CNAME auf den internen ALB (extern gepflegt); passt aufs `*.506.ai`-ACM-Cert am Listener → TLS-Verifikation bleibt strikt |

## Kunden-Token ausstellen

```bash
python3 deploy/mint_token.py --tenant <kunde>          # Rolle navigator, 1 Jahr
# liest das Secret aus dem Secrets Manager (Profil/Umgebung mit Leserecht)
```

Der Token kommt als `FEDLEX_MCP_TOKEN`, die URL als `FEDLEX_MCP_URL`
(`https://swiss-legal.506.ai:53000/mcp`) in die Secrets-Datei des Kunden
(secrets-service, Muster patentConsumerKey). Danach agentic-Container des
Kunden neu starten (Secrets-Cache!).

## Update von upstream einspielen

1. GitHub → **Sync fork** (holt mindful-bios neue Commits; `deploy/` bleibt unberührt)
2. Diff ansehen — was hat sich geändert? (CHANGELOG.md lesen)
3. Optional lokal testen: `cp .env.example .env && docker compose up --build`
4. Jenkins-Job „Swiss-Legal Deploy" starten → baut, pusht, registriert
   neue Task-Revision, `update-service`, wartet und **verifiziert**, dass
   nicht der Circuit-Breaker zurückgerollt hat
5. Rollback im Notfall: Service auf die vorherige Task-Revision zeigen

## Smoke-Test

```bash
# ohne Token → -32001 missing credential (fail-closed)
curl -s https://swiss-legal.506.ai:53000/rpc -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'

# mit Kunden-JWT → Tool-Liste
curl -s https://swiss-legal.506.ai:53000/rpc -H 'content-type: application/json' \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | head -c 300
```

## Historie

Ersetzt die EC2-Box `Swiss-Legal-Services` (i-0765580515dfb238e, 172.31.8.51)
vom 27.08.2026 — deren Compose-Setup war der Pilot; Terminierung nach
bewiesenem ECS-Betrieb. Plan: siehe Artifact „Swiss-Legal auf ECS".
