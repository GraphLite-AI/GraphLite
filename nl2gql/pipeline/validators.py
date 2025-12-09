from __future__ import annotations

import re
from typing import Dict, List, Optional, Tuple

from .config import DEFAULT_OPENAI_MODEL_FIX
from .ir import ISOQueryIR
from .openai_client import chat_complete
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

    def validate(self, nl: str, schema_summary: str, query: str, hints: List[str]) -> Tuple[bool, Optional[str]]:
        hint_text = "\n".join(f"- {h}" for h in hints) if hints else "none"
        user = (
            f"SCHEMA SUMMARY:\n{schema_summary}\n\n"
            f"NATURAL LANGUAGE:\n{nl}\n\n"
            f"QUERY:\n{query}\n\n"
            f"STRUCTURAL HINTS (optional):\n{hint_text}\n\n"
            "Does the query satisfy the request while staying faithful to the natural-language ask?"
        )
        # Keep logic gating strict but reduce randomness by using a single
        # low-temperature evaluation with a tighter nucleus sample.
        temps = [0.0]
        verdict, _ = chat_complete(self.model, self.SYSTEM, user, temperature=temps[0], top_p=0.3, max_tokens=160)
        verdict_upper = verdict.strip().upper()
        if verdict_upper.startswith("VALID"):
            return True, None
        if verdict_upper.startswith("INVALID:"):
            reason = verdict.strip()[len("INVALID:") :].strip() or "unspecified reason"
            return False, reason
        # Treat non-standard replies as uncertain but do not fabricate extra reasons.
        return False, verdict.strip() or "logic validator unsure"


__all__ = ["SchemaGroundingValidator", "LogicValidator"]


