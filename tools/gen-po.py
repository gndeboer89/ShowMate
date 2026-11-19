#!/usr/bin/env python3
"""Werkt de vertaalbestanden (lang/<taal>/LC_MESSAGES/showmate-app.po) bij vanuit de @tr()-teksten in ui/*.slint.

Gebruik: python3 tools/gen-po.py
- Bestaande Engelse vertalingen blijven behouden (opgezocht op msgid).
- Nieuwe teksten: voeg hieronder een Engelse vertaling toe in NEW_EN en draai het script opnieuw.
- Het script meldt teksten zonder vertaling en laat ze in dat geval ongewijzigd (Nederlands).
"""
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent / "crates" / "showmate-app"
NEW_EN = {
    "Uitgang": "Output",
    "Uitgang — onbekend": "Output — unknown",
    "Maak uitgangen aan en koppel elke uitgang aan een geluidskaart en twee kanalen. Cues kiezen een uitgang op naam.":
        "Create outputs and link each one to a sound card and two channels. Cues pick an output by name.",
    "Geluidskaart": "Sound card",
    "Links": "Left",
    "Rechts": "Right",
    "+ Uitgang": "+ Output",
    "De afspeelknop naast een uitgang speelt een testtoon. Een kaart wordt pas geopend als een uitgang of cue hem nodig heeft.":
        "The play button next to an output plays a test tone. A card is only opened when an output or cue needs it.",
    "Maak uitgangen aan en kies per uitgang op welke monitor(en) het beeld komt. Meerdere monitoren = hetzelfde beeld op allemaal.":
        "Create outputs and choose per output which monitor(s) show the picture. Several monitors = the same picture on all of them.",
    "Monitoren": "Monitors",
}

po_en = (root / "lang/en/LC_MESSAGES/showmate-app.po").read_text()
en = {k: v for k, v in re.findall(r'msgid "((?:[^"\\]|\\.)*)"\nmsgstr "((?:[^"\\]|\\.)*)"', po_en) if k}
en.update(NEW_EN)

def strings(path):
    text = path.read_text()
    comps = [(m.start(), m.group(1)) for m in re.finditer(r"component (\w+) inherits", text)]
    for m in re.finditer(r'@tr\("((?:[^"\\]|\\.)*)"\)', text):
        yield [n for p, n in comps if p < m.start()][-1], m.group(1)

entries = sorted({e for f in sorted((root / "ui").glob("*.slint")) for e in strings(f)})
missing = sorted({k for _, k in entries if k not in en})
if missing:
    print("zonder Engelse vertaling:", *missing, sep="\n  ", file=sys.stderr)

for lang in ("en", "nl"):
    lines = ['msgid ""', 'msgstr ""', '"Content-Type: text/plain; charset=UTF-8\\n"', f'"Language: {lang}\\n"', ""]
    for ctx, key in entries:
        value = en.get(key, key) if lang == "en" else key
        lines += [f'msgctxt "{ctx}"', f'msgid "{key}"', f'msgstr "{value}"', ""]
    (root / f"lang/{lang}/LC_MESSAGES/showmate-app.po").write_text("\n".join(lines))
print(f"{len(entries)} teksten bijgewerkt", file=sys.stderr)
