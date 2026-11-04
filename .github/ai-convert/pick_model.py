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


def gemini(listing: dict, wanted: str) -> str:
    """The newest Gemini Flash the key may call (Flash fits the free tier), else the newest Pro."""
    names = {
        m.get('name', '').removeprefix('models/')
        for m in listing.get('models', [])
        if 'generateContent' in m.get('supportedGenerationMethods', ['generateContent'])
    }
    if wanted and (not names or wanted in names):
        return wanted
    ranked = []
    for name in names:
        found = re.fullmatch(r'gemini-(\d+(?:\.\d+)?)-(flash|pro)(-preview(?:-[\w-]+)?)?', name)
        if found:
            version, kind, preview = float(found.group(1)), found.group(2), bool(found.group(3))
            # Flash before Pro, stable before preview, then the newest.
            ranked.append(((kind == 'flash', not preview, version), name))
    if not ranked:
        sys.exit('Gemini listed no Flash or Pro model this key can use: check GEMINI_API_KEY')
    return max(ranked)[1]


def groq(listing: dict, wanted: str) -> str:
    listed = {m['id'] for m in listing.get('data', []) if m.get('active', True)}
    if wanted and (not listed or wanted in listed):
        return wanted
    for model in PREFERRED:
        if model in listed:
            return model
    if not listed:
        sys.exit('Groq listed no models: check GROQ_API_KEY')
    return sorted(listed)[0]


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
    print(gemini(listing, wanted) if is_gemini else groq(listing, wanted))


if __name__ == '__main__':
    main()
