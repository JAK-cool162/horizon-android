#!/usr/bin/env python3
"""The id for a converted mod: the original's (a .hnmod's horizon-native.json, a jar's
fabric.mod.json, quilt.mod.json or [neo]forge mods.toml), else its file name, as Horizon's ids
are: lowercase letters, digits and `_`, 1-64 long, starting with a letter.

    mod_id.py <unpacked folder> <file name>
"""
import json
import re
import sys
from pathlib import Path


def original(root: Path) -> str:
    for name in ('horizon-native.json', 'fabric.mod.json'):
        try:
            return str(json.loads((root / name).read_text(encoding='utf-8', errors='replace'))['id'])
        except (OSError, ValueError, KeyError, TypeError):
            pass
    try:
        quilt = json.loads((root / 'quilt.mod.json').read_text(encoding='utf-8', errors='replace'))
        return str(quilt['quilt_loader']['id'])
    except (OSError, ValueError, KeyError, TypeError):
        pass
    for name in ('META-INF/neoforge.mods.toml', 'META-INF/mods.toml'):
        try:
            text = (root / name).read_text(encoding='utf-8', errors='replace')
        except OSError:
            continue
        found = re.search(r'^\s*modId\s*=\s*"([^"]+)"', text, re.M)
        if found:
            return found.group(1)
    return ''


def clean(text: str) -> str:
    text = re.sub(r'[^a-z0-9_]+', '_', text.lower()).strip('_')
    text = re.sub(r'_+', '_', text)
    if not text or not text[0].isalpha():
        text = 'mod_' + text
    return text[:64].rstrip('_') or 'converted_mod'


def main() -> None:
    root, file_name = Path(sys.argv[1]), sys.argv[2]
    found = original(root)
    if not found:
        # "mcw-bridges-3.1.2-mc1.21.1neoforge.jar" -> "mcw_bridges"
        stem = Path(file_name).stem
        found = re.split(r'[-_+](?:v?\d|mc\d)', stem, maxsplit=1)[0] or stem
    print(clean(found))


if __name__ == '__main__':
    main()
