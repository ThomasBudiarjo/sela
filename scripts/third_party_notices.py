#!/usr/bin/env python3
"""Generate THIRD-PARTY-NOTICES for the Sela installer (M5-02, M5-07).

The notices cover every crate compiled into the shipped binary for one target
(normal dependencies of `sela`, reached from Cargo.lock through
`cargo metadata`; build-only and dev-only crates are not distributed), the
assets embedded with `include_bytes!`, and the external programs the
installer bundles. Each crate's own license files are copied verbatim from
its package. A crate that ships none gets the standard SPDX text of each
license in its expression (scripts/notices/spdx/), with its authors named.

The output depends only on Cargo.lock, the crate sources it pins and the
files in this repository: no timestamps, no absolute paths, sorted order.
So the file is reproducible and `--check` fails when it is out of date.

Usage:
    python3 scripts/third_party_notices.py                # write the file
    python3 scripts/third_party_notices.py --check        # compare only
    python3 scripts/third_party_notices.py --target x86_64-unknown-linux-gnu
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPDX_DIR = ROOT / "scripts" / "notices" / "spdx"
DEFAULT_TARGET = "x86_64-pc-windows-msvc"
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice|copyright|unlicense)", re.IGNORECASE)
# Files that look like license files but hold none.
NOT_LICENSE = re.compile(r"\.(rs|toml|json|sh|py|in)$", re.IGNORECASE)

# Assets compiled into the binary and programs the installer ships beside it.
# Each entry: (title, license, origin, text file relative to the repository).
EMBEDDED = [
    (
        "DejaVu Sans and DejaVu Sans Bold 2.37 (embedded default fonts, src/fonts.rs)",
        "Bitstream Vera Fonts license; DejaVu changes are in the public domain",
        "https://dejavu-fonts.github.io/ (see tests/fixtures/README.md)",
        "tests/fixtures/DejaVuSans.LICENSE",
    ),
]
BUNDLED_PROGRAMS = """\
FFmpeg 8.1 (ffmpeg.exe, ffprobe.exe and their DLLs, when the installer
bundles them; see docs/media.md)
  License: LGPL-2.1-or-later. Sela only runs these programs as separate
  processes and does not link them. The installer ships FFmpeg's own LICENSE
  and README files in its ffmpeg folder, unmodified.
  Build: BtbN/FFmpeg-Builds, win64-lgpl-shared, release branch 8.1
  (asset ffmpeg-n8.1.3-9-g29e619e767-win64-lgpl-shared-8.1.zip).
  Corresponding source: https://github.com/FFmpeg/FFmpeg/tree/n8.1.3 and the
  build scripts at https://github.com/BtbN/FFmpeg-Builds.
"""


def cargo_metadata(target: str) -> dict:
    command = [
        "cargo",
        "metadata",
        "--locked",
        "--offline",
        "--format-version",
        "1",
        "--filter-platform",
        target,
        "--manifest-path",
        str(ROOT / "Cargo.toml"),
    ]
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    return json.loads(result.stdout)


def distributed(metadata: dict) -> list[dict]:
    """Packages reachable from the root through normal dependencies."""
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]
    seen: set[str] = set()
    stack = [root]
    while stack:
        package = stack.pop()
        if package in seen:
            continue
        seen.add(package)
        for dep in nodes[package]["deps"]:
            if any(kind["kind"] is None for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    seen.discard(root)
    return sorted((packages[i] for i in seen), key=lambda p: (p["name"], p["version"]))


def normalized(text: str) -> str:
    """Line endings and trailing spaces only; the wording is never changed."""
    lines = [line.rstrip() for line in text.replace("\r\n", "\n").replace("\r", "\n").split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines) + "\n"


def license_files(package: dict) -> list[tuple[str, str]]:
    directory = Path(package["manifest_path"]).parent
    names = set()
    if package.get("license_file"):
        names.add(package["license_file"])
    for entry in os.listdir(directory):
        if LICENSE_FILE.match(entry) and not NOT_LICENSE.search(entry):
            names.add(entry)
    found = []
    for name in sorted(names):
        path = directory / name
        if not path.is_file():
            continue
        text = path.read_bytes().decode("utf-8", errors="replace")
        if text.strip():
            found.append((name, normalized(text)))
    return found


def spdx_ids(expression: str) -> list[str]:
    tokens = re.split(r"[\s()/]+", expression.replace(" OR ", " ").replace(" AND ", " "))
    ids = []
    for token in tokens:
        if token and token not in ("OR", "AND", "WITH") and token not in ids:
            ids.append(token)
    return ids


def standard_texts(package: dict) -> list[tuple[str, str]]:
    expression = package.get("license") or ""
    texts = []
    for spdx in spdx_ids(expression):
        path = SPDX_DIR / f"{spdx}.txt"
        if path.is_file():
            texts.append((f"{spdx} (standard text)", normalized(path.read_text(encoding="utf-8"))))
    if not texts:
        raise SystemExit(
            f"{package['name']} {package['version']} ships no license file and no standard "
            f"text exists for {expression!r}; add one to {SPDX_DIR.relative_to(ROOT)}"
        )
    return texts


def source_of(package: dict) -> str:
    source = package.get("source") or ""
    if source.startswith("registry+"):
        return "crates.io"
    if source.startswith("git+"):
        url, _, rev = source[4:].partition("#")
        return f"{url.split('?')[0]} at {rev}"
    return "local"


def render(target: str, metadata: dict) -> str:
    packages = distributed(metadata)
    root = next(p for p in metadata["packages"] if p["id"] == metadata["resolve"]["root"])
    texts: dict[str, str] = {}
    users: dict[str, list[str]] = {}
    entries = []
    for package in packages:
        label = f"{package['name']} {package['version']}"
        files = license_files(package)
        standard = not files
        if standard:
            files = standard_texts(package)
        keys = []
        for name, text in files:
            key = hashlib.sha256(text.encode("utf-8")).hexdigest()[:12]
            texts.setdefault(key, text)
            users.setdefault(key, []).append(label)
            keys.append(f"{name} [{key}]")
        entries.append((package, keys, standard))

    out = []
    out.append("THIRD-PARTY SOFTWARE NOTICES FOR SELA\n")
    out.append("=" * 37 + "\n\n")
    out.append(
        f"Sela {root['version']} is licensed under {root['license']}; see LICENSE. Its source\n"
        f"code is available at {root.get('repository') or 'the Sela repository'}.\n\n"
        "This file lists the third-party software in the Sela program and installer\n"
        "and reproduces their license texts. It is generated by\n"
        "scripts/third_party_notices.py from Cargo.lock; do not edit it by hand.\n"
        f"Target: {target}. Rust crates compiled in: {len(packages)}.\n\n"
    )
    out.append("GPUI AND ZED CRATES\n-------------------\n\n")
    out.append(
        "The operator interface uses GPUI and supporting crates from the Zed\n"
        "editor (https://github.com/zed-industries/zed), pinned in Cargo.lock.\n"
        "They are licensed under the Apache License 2.0 with this notice\n"
        "(LICENSE-GPUI-APACHE):\n\n"
    )
    out.append(normalized((ROOT / "LICENSE-GPUI-APACHE").read_text(encoding="utf-8")))
    out.append("\nEMBEDDED ASSETS\n---------------\n\n")
    for title, license_name, origin, path in EMBEDDED:
        out.append(f"{title}\n  License: {license_name}\n  Origin: {origin}\n\n")
        out.append(normalized((ROOT / path).read_text(encoding="utf-8")))
        out.append("\n")
    out.append("PROGRAMS BUNDLED BY THE INSTALLER\n---------------------------------\n\n")
    out.append(BUNDLED_PROGRAMS)
    out.append("\nRUST CRATES\n-----------\n\n")
    out.append(
        "Each crate with its license expression, source and the license texts\n"
        "that apply to it (by key; the texts follow in the next section).\n"
        "Where a crate offers a choice of licenses, Sela may use any of them.\n\n"
    )
    for package, keys, standard in entries:
        out.append(f"{package['name']} {package['version']}\n")
        out.append(f"  License: {package.get('license') or 'see license file'}\n")
        out.append(f"  Source: {source_of(package)}\n")
        if package.get("repository"):
            out.append(f"  Repository: {package['repository']}\n")
        if standard:
            authors = ", ".join(package.get("authors") or []) or f"the {package['name']} authors"
            out.append(
                f"  The package ships no license file; the standard texts below apply,\n"
                f"  with the copyright held by: {authors}\n"
            )
        out.append(f"  Texts: {', '.join(keys)}\n\n")
    out.append("LICENSE TEXTS\n-------------\n\n")
    for key in sorted(texts, key=lambda k: (min(users[k]), k)):
        names = sorted(set(users[key]))
        out.append(f"[{key}] used by {len(names)} crate(s): {', '.join(names)}\n")
        out.append("-" * 72 + "\n")
        out.append(texts[key])
        out.append("-" * 72 + "\n\n")
    return "".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--target", default=DEFAULT_TARGET)
    parser.add_argument("--output", default=str(ROOT / "THIRD-PARTY-NOTICES"))
    parser.add_argument("--check", action="store_true", help="fail if the file is out of date")
    args = parser.parse_args()
    text = render(args.target, cargo_metadata(args.target))
    output = Path(args.output)
    if args.check:
        current = output.read_text(encoding="utf-8") if output.is_file() else ""
        if current != text:
            print(
                f"{output} is out of date; run python3 scripts/third_party_notices.py",
                file=sys.stderr,
            )
            return 1
        print(f"{output} is up to date ({len(text)} bytes)")
        return 0
    output.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {output} ({len(text)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
