"""Start one fakescan for the local matrix. Live tests still use an already-running server."""

from __future__ import annotations

import os
import socket
import subprocess
import time
from pathlib import Path

import httpx
import pytest

ROOT = Path(__file__).resolve().parents[2]


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _cargo_target_dir() -> Path:
    """Respect CARGO_TARGET_DIR so we do not run a stale ROOT/target binary."""
    env = os.environ.get("CARGO_TARGET_DIR", "").strip()
    return Path(env) if env else ROOT / "target"


def _ensure_binary() -> Path:
    subprocess.run(
        ["cargo", "build", "-p", "fakescan", "--bin", "fakescan"],
        cwd=ROOT,
        check=True,
    )
    name = "fakescan.exe" if os.name == "nt" else "fakescan"
    binary = _cargo_target_dir() / "debug" / name
    if not binary.is_file():
        raise RuntimeError(f"cargo build did not produce {binary}")
    return binary


@pytest.fixture(scope="session")
def fakescan_base_url() -> str:
    """fakescan on 127.0.0.1 with Scan Service routes and probe listen off.

    A distinct lab name and device UUID keep this process off the installed
    Fake scanner's pipe and WS-Discovery identity.
    """
    port = _free_port()
    binary = _ensure_binary()
    log_path = ROOT / "target" / "local-matrix.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log = log_path.open("w", encoding="utf-8")
    proc = subprocess.Popen(
        [
            str(binary),
            "--listen",
            f"127.0.0.1:{port}",
            "--profiles",
            "profiles",
            "--profile",
            "brother-wsd",
            "--lab-name",
            "local-matrix",
            "--device-uuid",
            "00000000-0000-4000-8000-00000000e11e",
            "--no-probe-listen",
        ],
        cwd=ROOT,
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    base = f"http://127.0.0.1:{port}"
    deadline = time.time() + 20
    up = False
    while time.time() < deadline:
        if proc.poll() is not None:
            log.flush()
            raise RuntimeError(f"fakescan exited {proc.returncode}; see {log_path}")
        try:
            response = httpx.get(f"{base}/health", timeout=0.4)
            if response.status_code == 200 and response.text.strip() == "ok":
                up = True
                break
        except httpx.HTTPError:
            time.sleep(0.1)
    if not up:
        proc.terminate()
        log.flush()
        raise RuntimeError(f"fakescan did not answer {base}/health; see {log_path}")
    os.environ["FAKESCAN_BASE_URL"] = base
    yield base
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
    log.close()
