from __future__ import annotations

import json
import re
from typing import Dict, List, Optional, Sequence, Tuple, Union

from .config import DEFAULT_OPENAI_MODEL_FIX
from .ir import ISOQueryIR
from .openai_client import chat_complete
from .requirements import RequirementContract, contract_view, resolve_required_output_forms
from .schema_graph import SchemaGraph


class SchemaGroundingValidator:
    def __init__(self, graph: SchemaGraph) -> None:
        self.graph = graph

    def validate(self, ir: ISOQueryIR) -> List[str]:
        errors: List[str] = []
        nodes = ir.nodes
        for alias, node in nodes.items():
            if node.label and node.label not in self.graph.nodes:
                errors.append(f"alias {alias} uses unknown label {node.label}")
        for edge in ir.edges:
            left_label = nodes.get(edge.left_alias, None).label if nodes.get(edge.left_alias) else None
            right_label = nodes.get(edge.right_alias, None).label if nodes.get(edge.right_alias) else None
            if left_label and right_label:
                if not any(e.src == left_label and e.rel == edge.rel and e.dst == right_label for e in self.graph.edges):
                    errors.append(f"edge not in schema: {left_label}-[:{edge.rel}]->{right_label}")
        for flt in ir.filters:
            node = nodes.get(flt.alias)
            if not node or not node.label:
                errors.append(f"filter references unknown alias {flt.alias}")
                continue
            if flt.prop not in self.graph.list_properties(node.label):
                errors.append(f"property {flt.prop} not on label {node.label}")
            if isinstance(flt.value, dict):
                ref_alias = flt.value.get("ref_alias")
                ref_prop = flt.value.get("ref_property")
                ref_node = nodes.get(ref_alias) if ref_alias else None
                if ref_node and ref_node.label:
                    if ref_prop not in self.graph.list_properties(ref_node.label):
                        errors.append(f"ref property {ref_prop} missing on {ref_node.label}")
        for ret in ir.returns:
            for alias_ref, prop in re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)", ret.expr):
                node = nodes.get(alias_ref)
                if not node or not node.label:
                    errors.append(f"return references unknown alias {alias_ref}")
                elif prop not in self.graph.list_properties(node.label):
                    errors.append(f"return references unknown property {prop} on {node.label}")
        errors.extend(ir.validate_bindings())
        return sorted(set(errors))


class LogicValidator:
    SYSTEM = (
        "You judge whether an ISO GQL query fully satisfies a natural-language request using only the provided schema summary. "
        "Hints are optional suggestions; ignore any hint that is not clearly required by the natural language request. "
        "Do NOT invent extra required entities or relationships beyond what the request asks for. "
        "Return 'VALID' if every requested constraint, join, grouping, aggregation, and ordering is present. "
        "Return 'INVALID: <reason>' if anything is missing or incorrect. Never propose a new query."
    )

    def __init__(self, model: str = DEFAULT_OPENAI_MODEL_FIX) -> None:
        self.model = model

    def validate(
        self,
        nl: str,
        schema_summary: Sequence[str] | str,
        query_or_ir: Union[str, ISOQueryIR],
        hints: List[str],
        *,
        contract: Optional[RequirementContract] = None,
    ) -> Tuple[bool, Optional[str]]:
        """
        Validate query semantics.

        Preferred mode (less noisy): pass `query_or_ir` as an `ISOQueryIR` plus the `contract`,
        so the judge sees the same structured artifact used by other stages.

        Backwards-compatible mode: pass `query_or_ir` as a rendered query string.
        """
        schema_text = schema_summary if isinstance(schema_summary, str) else "\n".join(schema_summary)
        hint_text = "\n".join(f"- {h}" for h in hints) if hints else "none"

        if isinstance(query_or_ir, ISOQueryIR) and contract is not None:
            output_forms = resolve_required_output_forms(contract, query_or_ir)
            payload = {
                "schema_summary": schema_text,
                "natural_language": nl,
                "hints": list(dict.fromkeys([h for h in hints if h])),
                "contract": contract_view(contract),
                "role_aliases": getattr(contract, "role_aliases", {}),
                "required_outputs_original": output_forms["original"],
                "required_outputs_alias": output_forms["alias"],
                "required_outputs_canonical": output_forms["alias_canonical"],
                "ast": query_or_ir.describe(),
            }
            user = json.dumps(payload, indent=2)
            system = (
                "You judge whether an ISO GQL AST satisfies a natural-language request.\n"
                "- Use the role_aliases + required_outputs_alias/canonical as authoritative.\n"
                "- Do not invent extra requirements beyond the request.\n"
                "- Respond as JSON: {\"result\":\"VALID\"} or {\"result\":\"INVALID\",\"reason\":\"...\"}.\n"
                "- Never propose a new query.\n"
            )
        else:
            user = (
                f"SCHEMA SUMMARY:\n{schema_text}\n\n"
                f"NATURAL LANGUAGE:\n{nl}\n\n"
                f"QUERY:\n{str(query_or_ir)}\n\n"
                f"STRUCTURAL HINTS (optional):\n{hint_text}\n\n"
                "Does the query satisfy the request while staying faithful to the natural-language ask?"
            )
            system = self.SYSTEM

        # Keep logic gating strict but reduce randomness by using a single
        # low-temperature evaluation with a tighter nucleus sample.
        temps = [0.0]
        verdict, _ = chat_complete(
            self.model,
            system,
            user,
            temperature=temps[0],
            top_p=0.3,
            max_tokens=160,
            force_json=True,
        )
        verdict_clean = verdict.strip()
        # Handle JSON replies like {"result":"VALID","reason": "..."}.
        try:
            data = json.loads(verdict_clean)
            if isinstance(data, dict):
                status = str(data.get("result") or data.get("status") or data.get("verdict") or "").strip().upper()
                reason_text = str(data.get("reason") or data.get("message") or "").strip()
                if status.startswith("VALID"):
                    return True, None
                if status.startswith("INVALID"):
                    return False, reason_text or verdict_clean or "logic validator unsure"
        except Exception:
            pass

        verdict_upper = verdict_clean.upper()
        if verdict_upper.startswith("VALID"):
            return True, None
        if verdict_upper.startswith("INVALID:"):
            reason = verdict_clean[len("INVALID:") :].strip() or "unspecified reason"
            return False, reason
        # Treat non-standard replies as uncertain but do not fabricate extra reasons.
        return False, verdict_clean or "logic validator unsure"


__all__ = ["SchemaGroundingValidator", "LogicValidator"]

