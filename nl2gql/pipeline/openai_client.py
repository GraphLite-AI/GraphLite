from __future__ import annotations

import threading
from typing import Any, Dict, Optional, Tuple

from tenacity import retry, stop_after_attempt, wait_fixed

from .config import DEFAULT_OPENAI_MODEL_FIX, DEFAULT_OPENAI_MODEL_GEN
from .utils import clean_block, safe_json_loads

_client_singleton = None
_USAGE_LOG = threading.local()


def _load_openai_client():
    try:
        from openai import OpenAI  # type: ignore
    except Exception as exc:  # pragma: no cover
        raise SystemExit("OpenAI client missing. Install with: pip install openai") from exc
    return OpenAI


def _client():
    global _client_singleton
    if _client_singleton is None:
        OpenAI = _load_openai_client()
        _client_singleton = OpenAI()
    return _client_singleton


def reset_usage_log() -> None:
    log = getattr(_USAGE_LOG, "entries", None)
    if log is None:
        _USAGE_LOG.entries = []
    else:
        log.clear()


def record_usage(usage: Dict[str, Any]) -> None:
    prompt = int(usage.get("prompt_tokens", 0))
    completion = int(usage.get("completion_tokens", 0))
    total = int(usage.get("total_tokens", prompt + completion))
    log = getattr(_USAGE_LOG, "entries", None)
    if log is None:
        log = []
        _USAGE_LOG.entries = log
    log.append({"prompt_tokens": prompt, "completion_tokens": completion, "total_tokens": total})


def usage_totals() -> Dict[str, int]:
    log = getattr(_USAGE_LOG, "entries", []) or []
    totals = {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
    for entry in log:
        totals["prompt_tokens"] += int(entry.get("prompt_tokens", 0))
        totals["completion_tokens"] += int(entry.get("completion_tokens", 0))
        totals["total_tokens"] += int(entry.get("total_tokens", 0))
    return totals


@retry(stop=stop_after_attempt(3), wait=wait_fixed(0.25))
def chat_complete(
    model: str,
    system: str,
    user: str,
    *,
    temperature: float = 0.15,
    top_p: float = 0.9,
    max_tokens: int = 700,
) -> Tuple[str, Optional[Dict[str, Any]]]:
    resp = _client().chat.completions.create(
        model=model,
        messages=[{"role": "system", "content": system}, {"role": "user", "content": user}],
        temperature=temperature,
        top_p=top_p,
        max_tokens=max_tokens,
    )

    text = (resp.choices[0].message.content or "").strip()
    usage_data = getattr(resp, "usage", None)
    if usage_data:
        usage = {
            "prompt_tokens": getattr(usage_data, "prompt_tokens", 0),
            "completion_tokens": getattr(usage_data, "completion_tokens", 0),
            "total_tokens": getattr(usage_data, "total_tokens", 0),
        }
        record_usage(usage)
        return text, usage
    return text, None


__all__ = [
    "chat_complete",
    "clean_block",
    "safe_json_loads",
    "reset_usage_log",
    "record_usage",
    "usage_totals",
    "DEFAULT_OPENAI_MODEL_GEN",
    "DEFAULT_OPENAI_MODEL_FIX",
]


