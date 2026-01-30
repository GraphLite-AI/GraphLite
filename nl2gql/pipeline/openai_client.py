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


def _openai_errors():
    try:
        from openai import APIStatusError, BadRequestError, NotFoundError  # type: ignore
        return (APIStatusError, BadRequestError, NotFoundError)
    except Exception:  # pragma: no cover
        return ()


def _is_max_tokens_unsupported(exc: Exception) -> bool:
    """Detect new-model errors that require max_completion_tokens instead of max_tokens."""
    text = str(exc).lower()
    return "max_tokens" in text and "max_completion_tokens" in text


def _is_temperature_unsupported(exc: Exception) -> bool:
    """Detect models that only allow the default temperature."""
    text = str(exc).lower()
    return "temperature" in text and "supported" in text


def _is_top_p_unsupported(exc: Exception) -> bool:
    """Detect models that disallow top_p overrides."""
    text = str(exc).lower()
    return "top_p" in text and "supported" in text


def _is_response_format_unsupported(exc: Exception) -> bool:
    """Detect models that do not support response_format."""
    text = str(exc).lower()
    return "response_format" in text and "supported" in text


def _requires_completion_tokens(model: str) -> bool:
    # GPT-5 nano (and similar) expect max_completion_tokens instead of max_tokens.
    return "gpt-5-nano" in model.lower()


def _requires_fixed_temperature(model: str) -> bool:
    # GPT-5 nano only accepts the default temperature (1) at the time of writing.
    return "gpt-5-nano" in model.lower()


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


@retry(stop=stop_after_attempt(3), wait=wait_fixed(0.25), reraise=True)
def chat_complete(
    model: str,
    system: str,
    user: str,
    *,
    temperature: float = 0.15,
    top_p: float = 0.9,
    max_tokens: int = 700,
    force_json: bool = False,
) -> Tuple[str, Optional[Dict[str, Any]]]:
    base_use_completion = _requires_completion_tokens(model)
    fixed_temp_model = _requires_fixed_temperature(model)
    # GPT-5 nano currently responds reliably via the Responses API.
    if base_use_completion:
        max_output_tokens = (
            max(max_tokens * 6, 8000) if max_tokens >= 800 else max(max_tokens * 6, 3000)
        )
        user_text = user if not force_json else f"{user}\n\nReturn JSON only."
        input_items = [
            {"type": "message", "role": "system", "content": [{"type": "input_text", "text": system}]},
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": user_text}]},
        ]
        params: Dict[str, Any] = {
            "model": model,
            "input": input_items,
            "max_output_tokens": max_output_tokens,
        }
        if force_json:
            params["text"] = {"format": {"type": "json_object"}}
        try:
            resp = _client().responses.create(**params)
        except _openai_errors() as exc:
            if force_json:
                params.pop("text", None)
                try:
                    resp = _client().responses.create(**params)
                except Exception as exc2:
                    raise RuntimeError(f"OpenAI model '{model}' is not available: {exc2}") from exc2
            else:
                raise RuntimeError(f"OpenAI model '{model}' is not available: {exc}") from exc
        text_blocks: list[str] = []
        for item in getattr(resp, "output", []) or []:
            content = getattr(item, "content", None) or []
            for chunk in content:
                if getattr(chunk, "type", "") == "output_text" and getattr(chunk, "text", None):
                    text_blocks.append(chunk.text)
        text = "\n".join(text_blocks).strip()
        usage_data = getattr(resp, "usage", None)
        if usage_data:
            usage = {
                "prompt_tokens": getattr(usage_data, "input_tokens", 0),
                "completion_tokens": getattr(usage_data, "output_tokens", 0),
                "total_tokens": getattr(usage_data, "total_tokens", 0),
            }
            record_usage(usage)
            return text, usage
        return text, None

    # Standard chat.completions path for other models.
    effective_max_tokens = max_tokens
    temp = None if fixed_temp_model else temperature
    top_p_val = None if fixed_temp_model else top_p

    user_payload = user if not force_json else f"{user}\n\nReturn JSON only."

    def _call(
        use_completion_tokens: bool,
        temp_override: Optional[float],
        top_p_override: Optional[float],
        json_mode: bool,
    ):
        token_param = (
            {"max_completion_tokens": effective_max_tokens}
            if use_completion_tokens
            else {"max_tokens": effective_max_tokens}
        )
        params: Dict[str, Any] = {
            "model": model,
            "messages": [{"role": "system", "content": system}, {"role": "user", "content": user_payload}],
            **token_param,
        }
        if temp_override is not None:
            params["temperature"] = temp_override
        if top_p_override is not None:
            params["top_p"] = top_p_override
        if json_mode:
            params["response_format"] = {"type": "json_object"}
        return _client().chat.completions.create(**params)

    try:
        resp = _call(
            use_completion_tokens=base_use_completion,
            temp_override=temp,
            top_p_override=top_p_val,
            json_mode=force_json,
        )
    except _openai_errors() as exc:
        if _is_max_tokens_unsupported(exc) and not base_use_completion:
            try:
                resp = _call(
                    use_completion_tokens=True,
                    temp_override=temp,
                    top_p_override=top_p_val,
                    json_mode=force_json,
                )
            except _openai_errors() as exc2:
                raise RuntimeError(f"OpenAI model '{model}' is not available: {exc2}") from exc2
        elif _is_temperature_unsupported(exc):
            try:
                resp = _call(
                    use_completion_tokens=base_use_completion,
                    temp_override=None,
                    top_p_override=top_p_val,
                    json_mode=force_json,
                )
            except _openai_errors() as exc2:
                raise RuntimeError(f"OpenAI model '{model}' is not available: {exc2}") from exc2
        elif _is_top_p_unsupported(exc):
            try:
                resp = _call(
                    use_completion_tokens=base_use_completion,
                    temp_override=temp,
                    top_p_override=None,
                    json_mode=force_json,
                )
            except _openai_errors() as exc2:
                raise RuntimeError(f"OpenAI model '{model}' is not available: {exc2}") from exc2
        elif _is_response_format_unsupported(exc) and force_json:
            try:
                resp = _call(
                    use_completion_tokens=base_use_completion,
                    temp_override=temp,
                    top_p_override=top_p_val,
                    json_mode=False,
                )
            except _openai_errors() as exc2:
                raise RuntimeError(f"OpenAI model '{model}' is not available: {exc2}") from exc2
        else:
            # Surface invalid/missing model errors without burying them in a RetryError.
            raise RuntimeError(f"OpenAI model '{model}' is not available: {exc}") from exc
    except Exception:
        # Let tenacity handle other transient errors.
        raise

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



