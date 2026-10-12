"""Optional pdfcraft-cli glue for WIA lab runs (never a hard dependency)."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path


def looks_like_fakescan(*, name: str = "", port: str = "", device_id: str = "") -> bool:
    joined = " ".join(p for p in (name, port, device_id) if p)
    if not joined:
        return False
    return bool(
        re.search(r"(?i)fakescan", joined)
        or re.search(r"(?i)Fake WSD scanner", joined)
        or re.search(r":3920", joined)
    )


def resolve_cli(explicit: str | Path | None = None) -> Path | None:
    if explicit:
        p = Path(explicit)
        if not p.is_file():
            raise FileNotFoundError(f"pdfcraft-cli not found: {p}")
        return p
    for name in ("pdfcraft-cli", "pdfcraft-cli.exe"):
        hit = shutil.which(name)
        if hit:
            return Path(hit)
    return None


def run_cli(exe: Path, args: list[str], *, timeout: float = 300.0) -> str:
    cmd = [str(exe), *args]
    print("pdfcraft-cli", *args)
    proc = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    out = (proc.stdout or "") + (proc.stderr or "")
    if proc.returncode != 0:
        raise RuntimeError(f"pdfcraft-cli exited {proc.returncode}: {out[-2000:]}")
    return out


def list_scanners(exe: Path) -> list[dict]:
    text = run_cli(exe, ["scan", "devices"])
    # devices command may print JSON or wrap it; take the outermost object.
    text = text.strip()
    try:
        obj = json.loads(text)
    except json.JSONDecodeError:
        start = text.find("{")
        end = text.rfind("}")
        if start < 0 or end <= start:
            raise RuntimeError(f"scan devices: not JSON:\n{text[:500]}") from None
        obj = json.loads(text[start : end + 1])
    return list(obj.get("scanners") or [])


def pick_fakescan_device(scanners: list[dict]) -> str | None:
    for row in scanners:
        name = str(row.get("name") or "")
        device_id = str(row.get("id") or "")
        port = str(row.get("port") or "")
        conn = str(row.get("connection") or "")
        looks = looks_like_fakescan(name=name, port=port, device_id=device_id)
        wsd_ok = conn == "" or bool(re.search(r"(?i)wsd", conn))
        if looks and wsd_ok:
            return name or device_id or None
    return None


def handling_status(row: dict) -> dict:
    """Return nested handling_status from a `scan devices` scanner row."""
    status = row.get("handling_status")
    return dict(status) if isinstance(status, dict) else {}


def find_device_row(scanners: list[dict], device: str) -> dict | None:
    wanted = device.casefold()
    for row in scanners:
        name = str(row.get("name") or "")
        device_id = str(row.get("id") or "")
        if name.casefold() == wanted or device_id.casefold() == wanted:
            return row
    return None


def scan_feeder(
    exe: Path,
    *,
    out_dir: Path,
    device: str,
    pages: int,
    dpi: int = 100,
) -> list[Path]:
    import shutil
    import time

    # Unique run dir so leftover acquires don't pollute compare.
    run_dir = out_dir / time.strftime("%Y%m%d-%H%M%S")
    if run_dir.exists():
        shutil.rmtree(run_dir)
    run_dir.mkdir(parents=True, exist_ok=True)
    run_cli(
        exe,
        [
            "scan",
            "--out",
            str(run_dir),
            "--yes",
            "--source",
            "feeder",
            "--pages",
            str(pages),
            "--dpi",
            str(dpi),
            "--device",
            device,
        ],
    )
    files = sorted(
        p
        for p in run_dir.iterdir()
        if p.is_file() and p.suffix.lower() in {".jpg", ".jpeg", ".jfif", ".png", ".bmp", ".tif", ".tiff"}
    )
    return files


def compare_acquired_to_originals(
    acquired: list[Path],
    originals: list[bytes],
    *,
    mode: str = "auto",
) -> None:
    """Compare acquired files to loopback originals.

    mode:
      - ``exact`` — byte-identical (SOAP / rare WIA paths)
      - ``pixels`` — decode with Pillow; mean abs channel delta must be small
      - ``auto`` — exact if equal, else pixels (WIA usually re-encodes JPEG)
    """
    if len(acquired) != len(originals):
        raise AssertionError(
            f"page count: acquired={len(acquired)} originals={len(originals)}"
        )
    for i, (path, original) in enumerate(zip(acquired, originals, strict=True)):
        got = path.read_bytes()
        if mode in ("exact", "auto") and got == original:
            print(f"page {i}: exact match ({path.name}, {len(got)} bytes)")
            continue
        if mode == "exact":
            raise AssertionError(
                f"page {i}: {path.name} not byte-identical "
                f"(acquired={len(got)} original={len(original)})"
            )
        _compare_pixels(i, path, got, original)


def _compare_pixels(index: int, path: Path, acquired: bytes, original: bytes) -> None:
    try:
        from PIL import Image
    except ImportError as e:
        raise AssertionError(
            f"page {index}: not byte-identical and Pillow missing for pixel compare "
            f"(pip install -e '.[dev]'). acquired={len(acquired)} original={len(original)}"
        ) from e
    import io

    a = Image.open(io.BytesIO(acquired)).convert("RGB")
    o = Image.open(io.BytesIO(original)).convert("RGB")
    if a.size != o.size:
        # WIA may resample; scale original to acquired size for a coarse check.
        o = o.resize(a.size, Image.Resampling.BILINEAR)
    ap = list(a.getdata())
    op = list(o.getdata())
    n = len(ap) * 3
    total = 0
    for (r1, g1, b1), (r2, g2, b2) in zip(ap, op, strict=True):
        total += abs(r1 - r2) + abs(g1 - g2) + abs(b1 - b2)
    mean = total / n
    # Loopback through WIA is lossy; allow moderate drift but catch wrong page.
    if mean > 25.0:
        raise AssertionError(
            f"page {index}: {path.name} pixel mean abs delta {mean:.1f} > 25 "
            f"(sizes acquired={a.size} original_scaled={o.size})"
        )
    print(f"page {index}: pixel match ({path.name}, mean_abs={mean:.1f})")


def default_out_dir() -> Path:
    base = os.environ.get("FAKESCAN_LAB_OUT")
    if base:
        return Path(base) / "loopback-wia"
    return Path.cwd() / "data" / "fakescan-lab" / "loopback-wia"
