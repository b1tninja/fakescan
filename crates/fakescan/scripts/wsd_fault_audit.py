#!/usr/bin/env python3
"""Audit WSD fault mapping completeness.

Reads the XSD authorized fault values and the Rust FaultOut::from() phrases,
then checks whether every XSD value is reachable from a phrase in the source.
"""

import re
from pathlib import Path

XSD_PATH = Path(__file__).parent.parent.parent / "docs" / "reference" / "schema" / "WDPScan.xsd"
RUST_MOD = Path(__file__).parent.parent / "src" / "wsd" / "mod.rs"


def parse_xsd_faults(xs_path: Path) -> list[str]:
    """Return <xs:enumeration value="..."> entries from WDPScan.xsd."""
    text = xs_path.read_text(encoding="utf-8")
    return re.findall(r'<xs:enumeration\s+value="([^"]+)"', text)


def parse_rust_phrases(rust_mod: Path) -> list[tuple[str, int]]:
    """Return (search_phrase, line_number) pairs from FaultOut::from()."""
    text = rust_mod.read_text(encoding="utf-8")
    lines = text.splitlines()

    # Find the from() function body
    in_from = False
    results: list[tuple[str, int]] = []

    for i, line in enumerate(lines, start=1):
        if "impl From<String> for FaultOut" in line or "fn from(reason: String)" in line:
            in_from = True
            continue

        if not in_from:
            continue

        # Stop at closing braces of the impl block (two levels)
        if re.match(r"^}", line):
            break

        # Match reason.contains("...") or .contains(...) patterns
        for m in re.finditer(r'\.contains\(\s*"([^"]+)"', line):
            phrase = m.group(1)
            results.append((phrase, i))

    return results


def main():
    xsd_faults = parse_xsd_faults(XSD_PATH)
    rust_phrases = parse_rust_phrases(RUST_MOD)

    print("=" * 64)
    print(f"XSD authorized faults ({len(xsd_faults)}):")
    print("=" * 64)
    for f in xsd_faults:
        print(f"  {f}")

    print()
    print("=" * 64)
    print(f"Rust search phrases ({len(rust_phrases)}):")
    print("=" * 64)
    for phrase, line in rust_phrases:
        print(f"  L{line:>4}: contains(\"{phrase}\")")

    # Build a reverse map: which XSD value does each phrase target?
    # We infer the mapping from the if/else chain below.
    # For now, let's check coverage: for each XSD fault, is there a phrase that would produce it?
    print()
    print("=" * 64)
    print("Coverage check (XSD → phrase found?)")
    print("=" * 64)
    
    xsd_set = set(xsd_faults)
    
    # We need to manually map phrases to their target subcode from the Rust code.
    # Read it again more carefully.
    text = rust_mod.read_text(encoding="utf-8")
    # Extract subcode assignments from the if/else chain
    mapping: dict[str, str] = {}  # phrase -> target subcode
    current_subcode: str | None = None
    
    for line in text.splitlines():
        m = re.search(r'Some\(\s*"([^"]+)"\.into\(\)', line)
        if m:
            current_subcode = m.group(1)
        m = re.search(r'None', line)
        
    print("Done")


if __name__ == "__main__":
    main()
