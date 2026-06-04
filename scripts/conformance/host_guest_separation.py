#!/usr/bin/env python3
"""Conformance gate: host/guest separation.

The `cowork` guest CLI is host-agnostic and must build for a Linux target with
no Windows-only dependencies. Windows-specific code lives only in the
`cowork-app` (src-tauri) host driver. This asserts the `cowork` package's full
dependency closure contains none of the known Windows-API crates, so the guest
binary ports as-is to any future host driver.

Usage: python3 scripts/conformance/host_guest_separation.py
Exits non-zero (and prints the offending path) on any violation.
"""

from __future__ import annotations

import json
import subprocess
import sys

GUEST_PACKAGE = "cowork"

# The guest runs inside WSL (Linux). We resolve dependencies for a Linux target
# so that Windows-only, `cfg(windows)`-gated transitive deps (e.g. clap pulls
# `windows-sys` only for terminal handling on Windows) are correctly excluded —
# they never compile for the guest. Only an *unconditional* Windows dependency
# would survive this filter, which is the real violation we want to catch.
# Only the x64 Linux target is checked. The guest's Windows-dependency surface
# does not vary by Linux arch (no crate pulls `windows-*` only on aarch64-linux),
# so a second target catches nothing new today. aarch64-unknown-linux-gnu (WSL on
# ARM64 Windows) is a deliberate future addition, not an oversight.
GUEST_TARGET = "x86_64-unknown-linux-gnu"

# Windows-API crates the guest must never pull in. Substring match on the crate
# name (covers windows, windows-sys, windows-core, windows-targets, winapi, ...).
FORBIDDEN_PREFIXES = ("windows", "winapi")


def load_metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", GUEST_TARGET],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(out.stdout)


def is_forbidden(name: str) -> bool:
    return any(name == p or name.startswith(p + "-") for p in FORBIDDEN_PREFIXES)


def main() -> int:
    meta = load_metadata()

    # Map package id -> name, and the resolve graph (id -> dependency ids).
    id_to_name = {p["id"]: p["name"] for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}

    guest_ids = [pid for pid, name in id_to_name.items() if name == GUEST_PACKAGE]
    if not guest_ids:
        print(f"FAIL: package '{GUEST_PACKAGE}' not found in workspace metadata", file=sys.stderr)
        return 2

    # BFS the dependency closure of the guest package.
    closure: set[str] = set()
    stack = list(guest_ids)
    while stack:
        pid = stack.pop()
        if pid in closure:
            continue
        closure.add(pid)
        for dep in nodes.get(pid, {}).get("deps", []):
            stack.append(dep["pkg"])

    violations = sorted(
        {id_to_name[pid] for pid in closure if pid in id_to_name and is_forbidden(id_to_name[pid])}
    )

    if violations:
        print("FAIL: host/guest separation violated.", file=sys.stderr)
        print(
            f"  The '{GUEST_PACKAGE}' guest CLI must not depend on Windows-API crates,",
            file=sys.stderr,
        )
        print("  but its dependency closure includes:", file=sys.stderr)
        for v in violations:
            print(f"    - {v}", file=sys.stderr)
        return 1

    print(f"OK: '{GUEST_PACKAGE}' dependency closure is free of Windows-API crates "
          f"({len(closure)} crates checked).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
