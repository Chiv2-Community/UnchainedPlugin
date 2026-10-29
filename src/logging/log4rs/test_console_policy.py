"""Run the real logger in isolated processes: log4rs caches its color policy."""
import errno
import os
import pty
import subprocess
import sys


def check(target, terminal, overrides, colored, custom=False):
    env = {k: v for k, v in os.environ.items()
           if k not in ("NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE")}
    env.update(overrides, UNCHAINED_TEST_CONSOLE_TARGET=target)
    env.pop("UNCHAINED_TEST_CUSTOM_PALETTE", None)
    if custom:
        env["UNCHAINED_TEST_CUSTOM_PALETTE"] = "1"
    master, slave = pty.openpty()
    try:
        # Keep the other stream terminal-backed, including redirected stderr.
        streams = {"stdout": slave, "stderr": slave}
        if not terminal:
            streams[target] = subprocess.PIPE
        child = subprocess.Popen(
            [sys.argv[1], "--exact", sys.argv[2], "--ignored", "--nocapture"],
            env=env, **streams)
        os.close(slave)
        slave = None
        stdout, stderr = child.communicate(timeout=20)
        transcript = bytearray()
        while True:
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break
            if not chunk:
                break
            transcript.extend(chunk)
        output = bytes(transcript) if terminal else {"stdout": stdout, "stderr": stderr}[target]
        case = (target, terminal, overrides)
        assert child.returncode == 0, (case, stdout, stderr, transcript)
        assert output.count(b"[policy_child            ] boundary message") == 5, (case, output)
        if colored:
            colors = ((b"17;34;51", b"34;51;68", b"51;68;85", b"68;85;102", b"85;102;119")
                      if custom else (b"242;139;130", b"242;204;120", b"140;180;225", b"128;128;128", b"92;92;92", b"238;238;238"))
            for rgb in colors:
                assert b"\x1b[0;38;2;" + rgb + b"m" in output, (case, output)
            assert output.count(b"\x1b[0m") >= 5, (case, output)
        else:
            assert b"\x1b" not in output, (case, output)
    finally:
        if slave is not None:
            os.close(slave)
        os.close(master)


for target in ("stdout", "stderr"):
    for terminal, overrides, colored in (
        (True, {}, True),
        (False, {}, False),
        (True, {"NO_COLOR": "1"}, False),
        (True, {"CLICOLOR": "0"}, False),
        (True, {"NO_COLOR": "1", "CLICOLOR_FORCE": "1"}, False),
        (True, {"NO_COLOR": "0"}, True),
        (False, {"CLICOLOR_FORCE": "1"}, True),
    ):
        check(target, terminal, overrides, colored)
    check(target, True, {}, True, custom=True)
    check(target, False, {}, False, custom=True)
    check(target, True, {"NO_COLOR": "1"}, False, custom=True)
