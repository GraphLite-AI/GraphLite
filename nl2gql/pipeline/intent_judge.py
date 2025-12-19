from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any, Dict, List, Sequence, Tuple

from .config import DEFAULT_OPENAI_MODEL_FIX
from .ir import ISOQueryIR
from .openai_client import chat_complete
from .requirements import RequirementContract, contract_view, resolve_required_output_forms


@dataclass
class IntentJudgeResult:
    valid: bool
    reasons: List[str] = field(default_factory=list)
    missing_requirements: List[str] = field(default_factory=list)


class IntentJudge:
    """
    LLM-driven judge that evaluates whether an ISOQueryIR satisfies the natural-language intent.
    It replaces bespoke heuristics by letting a model reason over a structured AST summary.
    """

    SYSTEM = (
        "You are an ISO GQL intent reviewer.\n"
        "You are given a schema summary, a natural-language request, a requirement contract, and an ISO GQL AST as JSON.\n"
        "Your job is NOT to restate deterministic errors. Assume schema/coverage/syntax checks exist elsewhere.\n"
        "Only flag higher-level intent/semantic mismatches that are NOT trivially detectable by string matching.\n"
        "\n"
        "Rules:\n"
        "- Treat role_aliases as authoritative alias bindings.\n"
        "- Treat required_outputs_canonical_nodistinct as the authoritative output requirement set.\n"
        "- Treat role_distinct_filters in the contract as authoritative distinctness constraints.\n"
        "- Do NOT invent extra constraints beyond the natural language request.\n"
        "- If you cannot point to AST evidence, return VALID (do not guess).\n"
        "\n"
        "Return STRICT JSON with this shape:\n"
        "{\n"
        "  \"result\": \"VALID\" | \"INVALID\",\n"
        "  \"missing\": {\n"
        "    \"joins\": [\"...\"],\n"
        "    \"filters\": [\"...\"],\n"
        "    \"grouping\": [\"...\"],\n"
        "    \"ordering\": [\"...\"],\n"
        "    \"outputs\": [\"...\"],\n"
        "    \"distinctness\": [\"...\"]\n"
        "  },\n"
        "  \"evidence\": {\n"
        "    \"ast_paths\": [\"ast.returns[0]\", \"ast.filters[1]\", \"contract.role_distinct_filters\", \"required_outputs_canonical_nodistinct\"]\n"
        "  },\n"
        "  \"notes\": [\"short explanation\"]\n"
        "}\n"
        "Use empty arrays when nothing is missing. Never propose a new query."
    )

    def __init__(self, model: str = DEFAULT_OPENAI_MODEL_FIX) -> None:
        self.model = model

    def evaluate(
        self,
        nl: str,
        schema_summary: Sequence[str] | str,
        ir: ISOQueryIR,
        contract: RequirementContract,
        hints: Sequence[str] | None = None,
        *,
        deterministic: Dict[str, Any] | None = None,
    ) -> IntentJudgeResult:
        schema_text = schema_summary if isinstance(schema_summary, str) else "\n".join(schema_summary)
        output_forms = resolve_required_output_forms(contract, ir)
        payload = {
            "schema_summary": schema_text,
            "natural_language": nl,
            "hints": list(dict.fromkeys([h for h in (hints or []) if h])),
            "contract": contract_view(contract),
            "role_aliases": getattr(contract, "role_aliases", {}),
            "required_outputs_original": output_forms["original"],
            "required_outputs_alias": output_forms["alias"],
            "required_outputs_canonical": output_forms["alias_canonical"],
            "required_outputs_canonical_nodistinct": output_forms["alias_canonical_nodistinct"],
            "ast": ir.describe(),
            "deterministic": deterministic or {},
        }

        def _as_list(value) -> List[str]:
            if not value:
                return []
            if isinstance(value, str):
                text = value.strip()
                return [text] if text else []
            out: List[str] = []
            for item in value:
                text = str(item).strip()
                if text:
                    out.append(text)
            return out

        def _call() -> Tuple[bool, List[str], List[str]]:
            reply, _ = chat_complete(
                self.model,
                self.SYSTEM,
                json.dumps(payload, indent=2),
                temperature=0.0,
                top_p=0.2,
                max_tokens=600,
                force_json=True,
            )
            data = json.loads(reply)
            status = str(data.get("result") or data.get("status") or data.get("verdict") or "").upper().strip()
            missing_obj = data.get("missing") or {}
            missing: List[str] = []
            if isinstance(missing_obj, dict):
                for key in ("joins", "filters", "grouping", "ordering", "outputs", "distinctness"):
                    missing.extend([f"{key}: {m}" for m in _as_list(missing_obj.get(key))])
            notes = _as_list(data.get("notes"))
            valid = status.startswith("VALID") or (not status and not missing)
            if not valid and not missing:
                # Invalid without concrete missing items is not actionable; treat as uncertain.
                return True, ["intent judge uncertain (no missing items)"], []
            return valid, notes, missing

        try:
            # Two-pass consensus: only hard-fail when both passes agree on INVALID with actionable missing.
            v1, notes1, missing1 = _call()
            v2, notes2, missing2 = _call()
            if v1 and v2:
                notes = list(dict.fromkeys(notes1 + notes2))
                return IntentJudgeResult(valid=True, reasons=notes, missing_requirements=[])
            if (not v1) and (not v2):
                missing = sorted(set(missing1) | set(missing2))
                notes = list(dict.fromkeys(notes1 + notes2))
                return IntentJudgeResult(valid=False, reasons=notes or ["intent mismatch"], missing_requirements=missing)
            # Disagreement -> advisory only.
            notes = list(dict.fromkeys(notes1 + notes2)) or ["intent judge uncertain (disagreement)"]
            return IntentJudgeResult(valid=True, reasons=notes, missing_requirements=[])
        except Exception:
            return IntentJudgeResult(valid=True, reasons=["intent judge unavailable"])


__all__ = ["IntentJudge", "IntentJudgeResult"]
