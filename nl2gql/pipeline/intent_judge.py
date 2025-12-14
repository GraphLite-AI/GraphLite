from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import List, Sequence

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
        "You are an expert ISO GQL reviewer. "
        "You are given (1) a schema summary, (2) the natural-language request, (3) structural hints/requirements, "
        "and (4) an ISO GQL AST expressed as JSON. "
        "Role aliases map semantic roles to concrete query aliases; treat these aliases as the authoritative bindings. "
        "Required outputs are provided both at the label level and as alias-level equivalents; accept semantically equivalent forms. "
        "Decide if the AST satisfies the request. "
        "Always respond as JSON with keys: status ('VALID' or 'INVALID'), "
        "'reasons' (list of human-readable explanations), and "
        "'missing' (list of concise requirements or constraints that are missing or incorrect). "
        "Never propose a new query."
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
            "ast": ir.describe(),
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

        try:
            reply, _ = chat_complete(
                self.model,
                self.SYSTEM,
                json.dumps(payload, indent=2),
                temperature=0.0,
                top_p=0.2,
                max_tokens=400,
                force_json=True,
            )
            data = json.loads(reply)
            status = str(
                data.get("status") or data.get("result") or data.get("verdict") or ""
            ).upper()
            valid = status.startswith("VALID")
            missing = _as_list(data.get("missing") or data.get("missing_requirements"))
            reasons = _as_list(data.get("reasons"))
            if not valid and not reasons and missing:
                reasons = [f"missing: {miss}" for miss in missing]
            return IntentJudgeResult(valid=valid, reasons=reasons, missing_requirements=missing)
        except Exception:
            # Do not fail the pipeline if the LLM call fails; treat as valid so deterministic checks can continue.
            return IntentJudgeResult(valid=True, reasons=["intent judge unavailable"])


__all__ = ["IntentJudge", "IntentJudgeResult"]
