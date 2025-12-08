from __future__ import annotations

import json
from dataclasses import dataclass
from typing import List, Optional

from .config import DEFAULT_OPENAI_MODEL_GEN
from .intent_linker import IntentLinkGuidance, links_to_hints
from .openai_client import chat_complete, clean_block, safe_json_loads


@dataclass
class CandidateQuery:
    query: str
    reason: Optional[str] = None
    usage: Optional[dict] = None


class QueryGenerator:
    SYSTEM = (
        "You are a cautious ISO GQL generator.\n"
        "- Use only schema labels/properties/relationships that appear in the provided filtered schema summary.\n"
        "- Do not invent names or traverse relationships that are not listed.\n"
        "- Use each relationship only between the labels it connects in the filtered schema summary and schema_links; preserve direction exactly as shown.\n"
        "- Prefer short lowercase aliases (n1, n2, p, t) instead of label names or reserved words; keep aliases consistent across MATCH/WHERE/WITH/RETURN.\n"
        "- Build a single MATCH with aliases, clear WHERE filters, explicit RETURN, ORDER BY, and LIMIT when requested.\n"
        "- Keep all graph patterns in MATCH (or subsequent MATCH statements); do not embed path patterns inside WHERE/RETURN.\n"
        "- Use properties only on their owning label; when you need related attributes, traverse to that node instead of inventing fields on another label.\n"
        "- Include only the nodes/relationships required to satisfy the request; avoid dangling nodes that are not grouped, filtered, or returned.\n"
        "- For comparisons across related nodes, create distinct aliases for each hop and compare their properties.\n"
        "- Prefer explicit traversals that follow the relationships given in the filtered schema summary rather than assuming shortcuts.\n"
        "- Use WITH when computing aggregates or rates; define derived metrics before filtering on them.\n"
        "- When counting entities, use COUNT(DISTINCT alias.id) when uniqueness matters; include HAVING-style filters via WITH/WHERE.\n"
        "- For ratios/percentages (shares, rates, drop-offs), compute numerator and denominator in WITH, derive the ratio, then filter/order.\n"
        "- Normalize relative dates as `date() - duration('P<n>D')` rather than vendor-specific date_sub/interval syntax.\n"
        "- Keep output to ISO GQL; avoid subqueries, CALL, or schema modifications.\n"
        "- Follow path hints and schema_links when they align with the request; reuse canonical aliases where provided.\n"
        "- Emit strictly the JSON shape requested."
    )

    USER_TEMPLATE = """Normalized NL: {nl}

Filtered schema:
{schema_summary}

Intent frame:
{intent_frame}

Schema links (grounded):
{links}

Preferred aliases (use as-is):
{alias_map}

Structural hints:
{hints}

Recent failures to avoid:
{failures}

Emit JSON:
{{
  "queries": [
    {{"query": "<ISO GQL text>", "reason": "concise plan"}},
    {{"query": "<alternate ISO GQL text>", "reason": "alternate plan"}}
  ]
}}
"""

    def __init__(self, model: str = DEFAULT_OPENAI_MODEL_GEN) -> None:
        self.model = model

    def generate(self, pre, failures: List[str], guidance: Optional[IntentLinkGuidance] = None) -> List[CandidateQuery]:
        failure_items = failures[-5:]
        failure_text = "- " + "\n- ".join(failure_items) if failure_items else "none"
        intent_frame = json.dumps(guidance.frame, indent=2) if guidance else "none"
        links_text = json.dumps(guidance.links, indent=2) if guidance else "none"
        alias_map = (
            json.dumps(
                {n["alias"]: n["label"] for n in guidance.links.get("node_links", []) if n.get("alias") and n.get("label")},
                indent=2,
            )
            if guidance
            else "none"
        )
        combined_hints = pre.structural_hints + (links_to_hints(guidance.links) if guidance else [])
        user = self.USER_TEMPLATE.format(
            nl=pre.normalized_nl,
            schema_summary=pre.filtered_schema.summary_lines(),
            intent_frame=intent_frame,
            links=links_text,
            alias_map=alias_map,
            hints="\n".join(sorted(set(combined_hints))) if combined_hints else "none",
            failures=failure_text,
        )
        raw, usage = chat_complete(self.model, self.SYSTEM, user, temperature=0.05, top_p=0.9, max_tokens=700)
        data = safe_json_loads(raw) or {}
        candidates: List[CandidateQuery] = []
        for entry in data.get("queries") or []:
            query = (entry.get("query") or "").strip()
            if query:
                candidates.append(CandidateQuery(query=query, reason=entry.get("reason"), usage=usage))
        if not candidates and raw.strip():
            candidates.append(CandidateQuery(query=clean_block(raw), usage=usage))
        return candidates


__all__ = ["QueryGenerator", "CandidateQuery"]


