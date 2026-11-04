#!/usr/bin/env python3
"""The Groq model to port with: the one asked for (argument 1, the repository variable AI_MODEL,
with or without its `groq/` prefix) when the key can use it, else the best coding model of
Groq's list on stdin (GET /openai/v1/models) by the order below. Groq's lineup changes, so the
list is read at run time instead of naming one model.

    curl .../openai/v1/models | pick_model.py [wanted]
"""
import json
import sys

# Strongest coding and long-context first.
PREFERRED = [
    'openai/gpt-oss-120b',
    'moonshotai/kimi-k2-instruct-0905',
    'moonshotai/kimi-k2-instruct',
    'qwen/qwen3.8-27b',
    'qwen/qwen3-32b',
    'llama-3.3-70b-versatile',
    'openai/gpt-oss-20b',
]


def main() -> None:
    wanted = (sys.argv[1] if len(sys.argv) > 1 else '').strip().removeprefix('groq/')
    try:
        listed = {m['id'] for m in json.load(sys.stdin).get('data', []) if m.get('active', True)}
    except (ValueError, AttributeError, TypeError):
        listed = set()
    if wanted and (not listed or wanted in listed):
        print(wanted)
        return
    for model in PREFERRED:
        if model in listed:
            print(model)
            return
    if not listed:
        sys.exit('Groq listed no models: check GROQ_API_KEY')
    print(sorted(listed)[0])


if __name__ == '__main__':
    main()
