#!/usr/bin/env python3
"""Exercise generated browser operations against Uvicorn on this machine."""

import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[1]


def main():
    subprocess.run(["cargo", "build", "--locked", "-p", "unchained-backend", "--examples"], cwd=ROOT, check=True)
    with tempfile.TemporaryDirectory(prefix="browser-interop-", dir="/tmp/opencode") as temporary:
        environment = {**os.environ, "CONFIG_DIR": temporary, "ADMIN_KEY": "local-test-admin", "TRUSTED_PROXY_CIDRS": ""}
        environment.pop("DISCORD_SIGNING_KEYRING", None)
        with socket.socket() as listener, (Path(temporary) / "backend.log").open("w+") as log:
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
                        with urlopen(base_url + "/api/v1/servers", timeout=1):
                            break
                    except OSError:
                        time.sleep(0.05)
                else:
                    raise RuntimeError("Local backend startup timed out")
                subprocess.run(["cargo", "run", "--locked", "--quiet", "-p", "unchained-backend", "--example", "browser-smoke", "--", base_url], cwd=ROOT, check=True)
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)
    print("Local backend/generated browser SDK interop passed")


if __name__ == "__main__":
    main()
