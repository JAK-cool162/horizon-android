#!/usr/bin/env python3
"""The model to port with: the one asked for (the repository variable AI_MODEL, with or without
its provider prefix) when the key can use it, else the best of the provider's list on stdin.
Providers retire models (Groq dropped Kimi K2; Gemini 2.5 Flash closed to new keys), so the list
is read at run time instead of naming one model.

    curl https://api.groq.com/openai/v1/models | pick_model.py [wanted]
    curl https://generativelanguage.googleapis.com/v1beta/models | pick_model.py --gemini [wanted]
"""
import json
import re
import sys

# Groq: the player's pick (Qwen 3.8) first, then the strongest coding and long-context models.
PREFERRED = [
    'qwen/qwen3.8-27b',
    'openai/gpt-oss-120b',
    'moonshotai/kimi-k2-instruct-0905',
    'moonshotai/kimi-k2-instruct',
    'qwen/qwen3-32b',
    'llama-3.3-70b-versatile',
    'openai/gpt-oss-20b',
]


def gemini(listing: dict, wanted: str) -> list:
    """Gemini models to try in turn: the newest Flash (it fits the free tier), the newest Flash
    Lite (another free quota, often idle when Flash is busy), then the newest Pro."""
    names = {
        m.get('name', '').removeprefix('models/')
        for m in listing.get('models', [])
        if 'generateContent' in m.get('supportedGenerationMethods', ['generateContent'])
    }
    if wanted and (not names or wanted in names):
        return [wanted]
    best = {}
    for name in names:
        found = re.fullmatch(r'gemini-(\d+(?:\.\d+)?)-(flash-lite|flash|pro)(-preview(?:-[\w-]+)?)?', name)
        if found:
            version, kind, stable = float(found.group(1)), found.group(2), not found.group(3)
            key = (stable, version)
            if kind not in best or key > best[kind][0]:
                best[kind] = (key, name)
    order = [best[k][1] for k in ('flash', 'flash-lite', 'pro') if k in best]
    if not order:
        sys.exit('Gemini listed no Flash or Pro model this key can use: check GEMINI_API_KEY')
    return order


def groq(listing: dict, wanted: str) -> list:
    """Groq models to try in turn, by PREFERRED."""
    listed = {m['id'] for m in listing.get('data', []) if m.get('active', True)}
    if wanted and (not listed or wanted in listed):
        return [wanted]
    order = [model for model in PREFERRED if model in listed]
    if not order and not listed:
        sys.exit('Groq listed no models: check GROQ_API_KEY')
    return order or sorted(listed)[:1]


def main() -> None:
    args = sys.argv[1:]
    is_gemini = bool(args) and args[0] == '--gemini'
    if is_gemini:
        args = args[1:]
    wanted = (args[0] if args else '').strip()
    wanted = wanted.removeprefix('gemini/' if is_gemini else 'groq/')
    try:
        listing = json.load(sys.stdin)
    except ValueError:
        listing = {}
    if not isinstance(listing, dict):
        listing = {}
    # One per line, the best first: the workflow moves on when one is busy or over its quota.
    print('\n'.join((gemini(listing, wanted) if is_gemini else groq(listing, wanted))[:3]))


if __name__ == '__main__':
    main()
