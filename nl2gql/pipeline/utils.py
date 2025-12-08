from __future__ import annotations

import difflib
import json
import re
from typing import Any, Iterable, List, Optional


def canonical(text: str) -> str:
    return re.sub(r"[^a-z0-9]", "", text.lower())


def tokenize(text: str) -> List[str]:
    return re.findall(r"[a-zA-Z][a-zA-Z0-9_]*", text)


def ratio(a: str, b: str) -> float:
    if not a or not b:
        return 0.0
    return difflib.SequenceMatcher(None, a, b).ratio()


def clean_block(text: str) -> str:
    stripped = text.strip()
    if stripped.startswith("```"):
        stripped = stripped[stripped.find("\n") + 1 :] if "\n" in stripped else stripped.lstrip("`")
    if stripped.endswith("```"):
        stripped = stripped[: stripped.rfind("```")]
    return stripped.strip()


def safe_json_loads(text: str) -> Optional[Any]:
    try:
        return json.loads(clean_block(text))
    except Exception:
        return None


