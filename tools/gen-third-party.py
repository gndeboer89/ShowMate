#!/usr/bin/env python3
"""Genereert de lijst met Rust-afhankelijkheden en hun licenties in THIRD-PARTY.md.

Gebruik (vanaf de repo-root, na wijzigingen in Cargo.lock):  python3 tools/gen-third-party.py
Alles tussen de markeringen BEGIN/END GENERATED wordt vervangen.
"""
import json
import subprocess
import sys

BEGIN = "<!-- BEGIN GENERATED -->"
END = "<!-- END GENERATED -->"

meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--locked"], text=True))
workspace = set(meta["workspace_members"])
rows = {}
for p in meta["packages"]:
    if p["id"] in workspace:
        continue
    lic = p.get("license") or ("zie " + p["license_file"].split("/")[-1] if p.get("license_file") else "onbekend")
    rows[(p["name"], p["version"])] = (lic.replace("/", " OR "), p.get("repository") or "")

lines = [f"{len(rows)} crates (transitief, uit `Cargo.lock`, voor alle doelplatformen; niet alles zit in de Windows-build):", "",
         "| Crate | Versie | Licentie |", "|---|---|---|"]
for (name, ver), (lic, repo) in sorted(rows.items()):
    lines.append(f"| {name} | {ver} | {lic} |")
block = "\n".join(lines)

path = "THIRD-PARTY.md"
text = open(path, encoding="utf-8").read()
a, b = text.index(BEGIN), text.index(END)
open(path, "w", encoding="utf-8").write(text[: a + len(BEGIN)] + "\n\n" + block + "\n\n" + text[b:])
print(f"{len(rows)} crates bijgewerkt in {path}", file=sys.stderr)
