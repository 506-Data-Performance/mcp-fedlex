#!/usr/bin/env python3
"""Swiss-Legal-Box — JWT fuer einen Kunden ausstellen (HS256, stdlib-only).

Laeuft auf der Box (liest MCP_JWT_HS256_SECRET + MCP_JWT_ISSUER aus .env):

    python3 mint_token.py --tenant mirus
    python3 mint_token.py --tenant staging --days 730

Der Token kommt danach als ``fedlexMcpToken`` in die Secrets-Datei des
Kunden (secrets-service, Muster patentConsumerKey). Rolle ist bewusst fest
``navigator`` (Suche + Lesen + Metadaten; keine Validation-Tools) — dieselbe
Rolle, mit der mindful.bios eigene Plattform laeuft.

Claims-Schema laut docs/dev/90_AUTH_AND_ROLES.md: iss (muss MCP_JWT_ISSUER
entsprechen), exp (Pflicht), tenant, sid, role. aud nur, wenn
MCP_JWT_AUDIENCE gesetzt ist.
"""
import argparse
import base64
import hashlib
import hmac
import json
import os
import sys
import time
from pathlib import Path


def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def _load_env(path: Path) -> dict:
    env: dict = {}
    if path.is_file():
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, _, v = line.partition("=")
                env[k.strip()] = v.strip()
    env.update(os.environ)
    return env


def main() -> int:
    ap = argparse.ArgumentParser(description="HS256-JWT fuer mcp-fedlex ausstellen")
    ap.add_argument("--tenant", required=True,
                    help="Kundenkennung (customer_list.json-Key), z.B. mirus")
    ap.add_argument("--days", type=int, default=365, help="Gueltigkeit in Tagen")
    ap.add_argument("--sid", default=None,
                    help="Session-Claim (Default: cgpt-<tenant>)")
    ap.add_argument("--env-file", default=".env", help="Pfad zur Box-.env")
    args = ap.parse_args()

    env = _load_env(Path(args.env_file))
    secret = env.get("MCP_JWT_HS256_SECRET", "")
    issuer = env.get("MCP_JWT_ISSUER", "https://506.ai/swiss-legal")
    audience = env.get("MCP_JWT_AUDIENCE", "mcp-fedlex")
    if not secret:
        # ECS-Betrieb: Secret liegt im AWS Secrets Manager (deploy/README.md).
        import subprocess
        try:
            secret = subprocess.run(
                ["aws", "secretsmanager", "get-secret-value",
                 "--secret-id", "SWISS_LEGAL_HS256_SECRET",
                 "--query", "SecretString", "--output", "text"],
                capture_output=True, text=True, check=True,
            ).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            secret = ""
    if not secret or not issuer:
        print("FEHLER: MCP_JWT_HS256_SECRET und MCP_JWT_ISSUER muessen in der "
              ".env stehen (fail-closed, kein Default).", file=sys.stderr)
        return 1

    now = int(time.time())
    payload = {
        "iss": issuer,
        "iat": now,
        "exp": now + args.days * 86400,
        "tenant": args.tenant,
        "sid": args.sid or f"cgpt-{args.tenant}",
        "role": "navigator",
    }
    if audience:
        payload["aud"] = audience

    header = {"alg": "HS256", "typ": "JWT"}
    signing_input = (_b64url(json.dumps(header, separators=(",", ":")).encode())
                     + "." +
                     _b64url(json.dumps(payload, separators=(",", ":")).encode()))
    sig = hmac.new(secret.encode(), signing_input.encode(), hashlib.sha256).digest()
    token = signing_input + "." + _b64url(sig)

    exp_date = time.strftime("%Y-%m-%d", time.gmtime(payload["exp"]))
    print(f"# tenant={args.tenant} role=navigator exp={exp_date}", file=sys.stderr)
    print(token)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
