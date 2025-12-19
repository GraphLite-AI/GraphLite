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


def safe_json_loads(text: str, *, merge_duplicate_keys: bool = False) -> Optional[Any]:
    """
    Load JSON while optionally merging duplicate top-level keys that some LLMs
    emit (e.g., two separate \"with\" blocks). When merging, list-valued fields
    are concatenated to avoid silent overwrites.
    """
    def _merge_pairs(pairs: List[tuple[str, Any]]) -> dict:
        merged: dict = {}
        list_keys = {
            "match",
            "where",
            "having",
            "with",
            "with_items",
            "group_by",
            "return",
            "order_by",
            "limit",
        }
        for key, value in pairs:
            if merge_duplicate_keys and key in list_keys:
                existing = merged.get(key)
                if existing is None:
                    merged[key] = value if isinstance(value, list) else ([value] if value is not None else [])
                    continue
                if not isinstance(existing, list):
                    existing = [existing]
                if isinstance(value, list):
                    existing.extend(value)
                elif value is not None:
                    existing.append(value)
                merged[key] = existing
                continue
            if merge_duplicate_keys and key in merged and isinstance(merged[key], dict) and isinstance(value, dict):
                combined = dict(merged[key])
                combined.update(value)
                merged[key] = combined
                continue
            merged[key] = value
        return merged

    try:
        return json.loads(clean_block(text), object_pairs_hook=_merge_pairs if merge_duplicate_keys else None)
    except Exception:
        return None


