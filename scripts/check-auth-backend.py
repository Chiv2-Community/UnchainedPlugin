#!/usr/bin/env python3
"""Run with the backend's uv environment; host all HTTP services on loopback."""

import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
from urllib.request import urlopen
from uuid import uuid4

from server_browser_backend.auth.signing import SigningKeys, provision

ROOT = Path(__file__).resolve().parents[1]


def main():
    # Compile before opening the listener so compilation never consumes ticket lifetime.
    subprocess.run(["cargo", "build", "--locked", "-p", "unchained-auth", "-p", "plugin-portable-tests", "--examples"], cwd=ROOT, check=True)
    with tempfile.TemporaryDirectory(prefix="auth-interop-", dir="/tmp/opencode") as temporary:
        directory = Path(temporary)
        keyring = directory / "signing.json"
        provision(keyring)
        environment = {
            **os.environ,
            "CONFIG_DIR": temporary,
            "ADMIN_KEY": "local-test-admin",
            "DISCORD_SIGNING_KEYRING": str(keyring),
            "TRUSTED_PROXY_CIDRS": "",
        }
        with socket.socket() as listener, (directory / "backend.log").open("w+") as log:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            base_url = f"http://127.0.0.1:{listener.getsockname()[1]}"
            server = subprocess.Popen([
                sys.executable, "-m", "uvicorn", "server_browser_backend.main:app",
                "--fd", str(listener.fileno()), "--no-proxy-headers", "--no-access-log",
            ], env=environment, pass_fds=(listener.fileno(),), stdout=log, stderr=log)
            try:
                for _ in range(100):
                    if server.poll() is not None:
                        raise RuntimeError("Local backend exited during startup")
                    try:
                        with urlopen(base_url + "/api/v1/auth/jwks.json", timeout=1) as response:
                            jwks = json.load(response)
                        break
                    except OSError:
                        time.sleep(0.05)
                else:
                    raise RuntimeError("Local backend startup timed out")
                subprocess.run(["cargo", "run", "--locked", "--quiet", "-p", "plugin-portable-tests", "--example", "browser-smoke", "--", base_url], cwd=ROOT, check=True)
                now = int(time.time())
                registration = str(uuid4())
                # HTTPS issuer identity is independent of the loopback test transport.
                issuer = "https://local-auth.test"
                ticket = SigningKeys(keyring).issue_join_ticket(
                    issuer=issuer, discord_id="123456789012345678",
                    server_id=registration, auth_time=now,
                )
                result = subprocess.run([
                    "cargo", "run", "--locked", "--quiet", "-p", "unchained-auth", "--example", "verify-ticket",
                ], cwd=ROOT, input=json.dumps({
                    "issuer": issuer, "server_id": registration, "jwks": jwks, "ticket": ticket, "now": now,
                }), text=True, capture_output=True, check=True)
                assert "replay rejected" in result.stdout
                print(result.stdout.strip())
                log.flush()
                log.seek(0)
                assert ticket not in log.read(), "Ticket leaked to backend log"
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)
    print("Local backend/generated Rust SDK/signing interop passed; no Discord service used")


if __name__ == "__main__":
    main()
