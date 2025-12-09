from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import List, Optional, Set, Tuple

from .config import DEFAULT_OPENAI_MODEL_GEN
from .intent_linker import IntentLinkGuidance, links_to_hints
from .requirements import RequirementContract, contract_view
from .openai_client import chat_complete, clean_block, safe_json_loads


@dataclass
class CandidateQuery:
    query: str
    reason: Optional[str] = None
    usage: Optional[dict] = None


@dataclass
class Plan:
    match: List[str]
    where: List[str]
    having: List[str]
    with_items: List[str]
    group_by: List[str]
    returns: List[str]
    order_by: List[str]
    limit: Optional[int]
    reason: Optional[str]

    @staticmethod
    def _collect_aliases(match_lines: List[str]) -> Set[str]:
        aliases: Set[str] = set()
        for m in match_lines:
            tokens = m.replace("(", " ").replace(")", " ").replace(",", " ").split()
            for token in tokens:
                if ":" in token:
                    parts = token.split(":")
                    if parts[0]:
                        aliases.add(parts[0])
        return aliases

    @staticmethod
    def _expr_aliases(expr: str) -> Set[str]:
        aliases: Set[str] = set()
        import re

        for alias, _prop in re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\\.([A-Za-z_][A-Za-z0-9_]*)", expr):
            aliases.add(alias)
        return aliases

    @classmethod
    def from_raw(cls, data: dict) -> Optional["Plan"]:
        if not isinstance(data, dict):
            return None

        # Normalize misplaced clauses that sometimes leak into the match list.
        raw_match = data.get("match") or []
        where_from_match: List[str] = []
        having_from_match: List[str] = []
        with_from_match: List[str] = []
        cleaned_match: List[str] = []
        for item in raw_match if isinstance(raw_match, list) else []:
            val = str(item).strip()
            lower = val.lower()
            if lower.startswith("where "):
                where_from_match.append(val[5:].strip())
                continue
            if lower.startswith("having "):
                having_from_match.append(val[6:].strip())
                continue
            if lower.startswith("with "):
                with_from_match.append(val[4:].strip())
                continue
            cleaned_match.append(val)
        if cleaned_match:
            data["match"] = cleaned_match
        if where_from_match:
            data["where"] = (data.get("where") or []) + where_from_match
        if having_from_match:
            data["having"] = (data.get("having") or []) + having_from_match
        if with_from_match:
            data["with"] = (data.get("with") or []) + with_from_match

        def _clean_list(key: str) -> List[str]:
            """
            Normalize list-valued fields from the model output while staying permissive
            enough to keep GQL patterns like [:REL] and IN [...] literals.
            """
            items = data.get(key, [])
            if not isinstance(items, list):
                return []
            cleaned: List[str] = []
            for x in items:
                val = str(x).strip()
                if not val:
                    continue
                if val.lower().startswith(("where ", "having ", "with ", "return ", "order by ")):
                    continue
                # Skip only obviously broken / gigantic fragments.
                if len(val) > 240:
                    continue
                cleaned.append(val)
            return cleaned

        match = _clean_list("match")
        returns = _clean_list("return")
        if not match or not returns:
            return None

        where = _clean_list("where")
        having = _clean_list("having")
        with_items = _clean_list("with")
        group_by = _clean_list("group_by")
        order_by = _clean_list("order_by")
        limit_raw = data.get("limit")
        limit = int(limit_raw) if isinstance(limit_raw, int) and limit_raw > 0 else None
        reason = data.get("reason")

        aliases = cls._collect_aliases(match)

        def _all_aliases_known(exprs: List[str]) -> bool:
            for expr in exprs:
                if not expr:
                    continue
                refs = cls._expr_aliases(expr)
                if refs and not refs.issubset(aliases):
                    return False
            return True

        if not _all_aliases_known(returns + order_by + with_items + group_by + where):
            return None

        return cls(
            match=match,
            where=where,
            having=having,
            with_items=with_items,
            group_by=group_by,
            returns=returns,
            order_by=order_by,
            limit=limit,
            reason=reason if isinstance(reason, str) else None,
        )

    def render(self) -> str:
        lines: List[str] = []
        seen_match: set[str] = set()
        deduped_match: List[str] = []
        for m in self.match:
            if m in seen_match:
                continue
            seen_match.add(m)
            deduped_match.append(m)

        for m in deduped_match:
            lines.append(f"MATCH {m}")
        if self.where:
            lines.append("WHERE " + " AND ".join(self.where))

        # Normalize grouping to avoid duplicate aliases like "WITH category, n4.category AS category".
        with_clause_parts: List[str] = []
        # Prefer explicit WITH items; use group_by entries only when they are not already represented.
        explicit_with = list(self.with_items)
        group_fill: List[str] = []
        known_aliases: set[str] = set()

        def _alias_of(expr: str) -> Optional[str]:
            parts = re.split(r"\s+AS\s+", expr, flags=re.IGNORECASE)
            return parts[1].strip() if len(parts) == 2 else None

        for item in explicit_with:
            alias = _alias_of(item)
            if alias:
                known_aliases.add(alias)

        for grp in self.group_by:
            # Skip if grouping already covered by an alias in WITH.
            alias = _alias_of(grp)
            if (alias and alias in known_aliases) or grp in known_aliases:
                continue
            if grp in explicit_with:
                continue
            group_fill.append(grp)

        merged_with = group_fill + explicit_with

        seen_parts: set[str] = set()
        seen_aliases: set[str] = set()
        for item in merged_with:
            alias = _alias_of(item)
            if alias and alias in seen_aliases:
                continue
            if item in seen_parts:
                continue
            seen_parts.add(item)
            if alias:
                seen_aliases.add(alias)
            with_clause_parts.append(item)

        if with_clause_parts:
            with_line = "WITH " + ", ".join(with_clause_parts)
            lines.append(with_line)
            if self.having:
                lines.append("WHERE " + " AND ".join(self.having))

        lines.append("RETURN " + ", ".join(self.returns))
        if self.order_by:
            lines.append("ORDER BY " + ", ".join(self.order_by))
        if isinstance(self.limit, int) and self.limit > 0:
            lines.append(f"LIMIT {self.limit}")
        return "\n".join(lines)


class QueryGenerator:
    SYSTEM = (
        "You are a cautious ISO GQL planner and renderer.\n"
        "- Use only schema labels/properties/relationships from the filtered schema summary; never invent names.\n"
        "- Preserve relationship direction exactly as shown; do not duplicate the same rel for both origin/destination unless the schema explicitly has both.\n"
        "- Do not repeat the same MATCH pattern; each edge should appear once, with distinct aliases when the NL differentiates roles (e.g., origin vs destination).\n"
        "- Avoid alias collisions: never reuse one alias for different labels or roles; prefer n1/n2/n3, origin/destination, src/dst when roles differ.\n"
        "- Prefer short lowercase aliases (n1, n2, p, t); keep aliases consistent across MATCH/WHERE/WITH/RETURN.\n"
        "- Every required label/edge/property from the contract MUST appear in MATCH/WHERE/RETURN/ORDER as appropriate; do not omit them.\n"
        "- Build clear MATCH blocks, THEN use WITH for aggregates/ratios, THEN RETURN/ORDER/LIMIT. Do not mix aggregated and non-aggregated expressions without grouping; always GROUP (or WITH) by every non-aggregated expression you return.\n"
        "- Ensure the plan is COMPLETE: include RETURN (and ORDER/LIMIT when required); never truncate or leave dangling MATCH/WHERE/WITH.\n"
        "- When counting, use COUNT(DISTINCT alias.id) if uniqueness matters; include HAVING-style filters via WITH + WHERE and ensure grouping aliases are carried in WITH.\n"
        "- For ratios/percentages (share/rate), compute numerator and denominator in WITH, derive the ratio, then filter/order on that alias; keep both numerator/denominator visible.\n"
        "- Normalize relative dates as `date() - duration('P<n>D')`.\n"
        "- RETURN ONLY the fields requested (targets + metrics); do NOT include extra ids or intermediate fields; include requested metrics/aggregates explicitly.\n"
        "- Avoid dangling nodes/edges that are not filtered, aggregated, or returned; every MATCHed alias should be used.\n"
        "- Keep output ISO GQL only; no subqueries, no CALL, no schema changes.\n"
        "- Follow path hints and schema_links when they align with the request; reuse canonical aliases when provided.\n"
        "- Prefer shortest valid traversals from the hints; if two roles use the same label (e.g., Airport origin vs destination), use different aliases and the correct rel directions.\n"
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

Hard requirements (must appear in the query):
{contract_text}

Recent failures to avoid:
{failures}

Emit JSON:
{{
  "match": ["(n1:Label)-[:REL]->(n2:OtherLabel)"],
  "where": ["condition1", "condition2"],
  "with": ["agg_expr AS agg_alias"],
  "group_by": ["n1", "n2.prop"],
  "return": ["expr1 AS alias1", "expr2"],
  "order_by": ["expr1 DESC", "expr2 ASC"],
  "limit": 5,
  "reason": "primary plan"
}}
"""

    def __init__(self, model: str = DEFAULT_OPENAI_MODEL_GEN) -> None:
        self.model = model

    def generate(
        self,
        pre,
        failures: List[str],
        guidance: Optional[IntentLinkGuidance] = None,
        contract: Optional[RequirementContract] = None,
        trace: Optional[dict] = None,
    ) -> List[CandidateQuery]:
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
        contract_text = json.dumps(contract_view(contract), indent=2) if contract else "none"
        user = self.USER_TEMPLATE.format(
            nl=pre.normalized_nl,
            schema_summary=pre.filtered_schema.summary_lines(),
            intent_frame=intent_frame,
            links=links_text,
            alias_map=alias_map,
            hints="\n".join(sorted(set(combined_hints))) if combined_hints else "none",
            contract_text=contract_text,
            failures=failure_text,
        )
        if trace is not None:
            trace["prompt"] = user
        raw, usage = chat_complete(self.model, self.SYSTEM, user, temperature=0.0, top_p=0.2, max_tokens=1200)
        if trace is not None:
            trace["raw"] = raw
        data = safe_json_loads(raw) or {}

        candidates: List[CandidateQuery] = []
        plan = Plan.from_raw(data) if isinstance(data, dict) else None
        if plan:
            candidates.append(CandidateQuery(query=plan.render(), reason=plan.reason, usage=usage))
        elif raw.strip():
            # As a last resort, treat cleaned text as query (kept for debuggability).
            candidates.append(CandidateQuery(query=clean_block(raw), usage=usage))
        return candidates


__all__ = ["QueryGenerator", "CandidateQuery"]


