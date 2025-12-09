from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Optional, Set, Tuple
import re

from .schema_graph import SchemaGraph


@dataclass
class RequirementContract:
    required_labels: Set[str] = field(default_factory=set)
    required_edges: Set[Tuple[str, str, str]] = field(default_factory=set)  # (src_label, rel, dst_label)
    required_properties: Set[Tuple[str, str]] = field(default_factory=set)  # (label, property)
    required_metrics: List[str] = field(default_factory=list)
    required_order: List[str] = field(default_factory=list)
    limit: Optional[int] = None
    # Roles that should be represented by distinct aliases (e.g., ORIGIN vs DESTINATION).
    required_distinct_roles: Set[Tuple[str, str, str, str]] = field(default_factory=set)
    # Tokens that should appear with an explicit zero/absence constraint (heuristic).
    required_zero_terms: Set[str] = field(default_factory=set)
    # Unordered pairs mentioned as "between A and B" that should be covered symmetrically.
    required_bidirectional_values: Set[Tuple[str, str]] = field(default_factory=set)
    # Normalized filter tokens the NL explicitly asked for (keywords, entities).
    required_filter_terms: Set[str] = field(default_factory=set)
    # Numeric literals the NL uses in constraints (e.g., thresholds, limits).
    required_numeric_literals: Set[str] = field(default_factory=set)
    # Numeric comparisons extracted from filters: (literal, comparator token/op).
    required_numeric_comparisons: Set[Tuple[str, str]] = field(default_factory=set)


def build_contract(nl: str, pre, guidance, graph: SchemaGraph) -> RequirementContract:
    """Derive structural and metric expectations from intent+linking output (schema-aware)."""
    contract = RequirementContract()

    links = guidance.links or {}
    node_links = links.get("node_links") or []
    rel_links = links.get("rel_links") or []
    prop_links = links.get("property_links") or []

    # Heuristic detection of "zero/no X" asks in NL to require explicit zero-count handling.
    lowered_nl = pre.normalized_nl.lower() if hasattr(pre, "normalized_nl") else nl.lower()
    for match in re.finditer(r"\b(no|zero|without)\s+([a-z0-9_]+)\b", lowered_nl):
        term = match.group(2).rstrip("s")  # normalize simple plurals like "comments"
        if term:
            contract.required_zero_terms.add(term)
            contract.required_filter_terms.add(term)

    # Heuristic detection of "between A and B" phrasing to require bidirectional coverage.
    for match in re.finditer(r"\bbetween\s+([a-z0-9_' -]+?)\s+and\s+([a-z0-9_' -]+)\b", lowered_nl):
        left = match.group(1).strip(" '")
        right = match.group(2).strip(" '")
        if left and right and left != right:
            pair = tuple(sorted([left, right]))
            contract.required_bidirectional_values.add(pair)  # type: ignore[arg-type]

    alias_to_label: Dict[str, str] = {}
    for nl in node_links:
        alias, label = nl.get("alias"), nl.get("label")
        if alias and label and graph.has_node(label):
            alias_to_label[alias] = label
            contract.required_labels.add(label)

    for rl in rel_links:
        left, rel, right = rl.get("left_alias"), rl.get("rel"), rl.get("right_alias")
        if not (left and rel and right):
            continue
        left_label = alias_to_label.get(left)
        right_label = alias_to_label.get(right)
        if left_label and right_label and graph.edge_exists(left_label, rel, right_label):
            contract.required_edges.add((left_label, rel, right_label))

    for pl in prop_links:
        alias, prop = pl.get("alias"), pl.get("property")
        label = alias_to_label.get(alias)
        if label and prop and graph.has_property(label, prop):
            contract.required_properties.add((label, prop))

    frame = guidance.frame or {}
    metrics = frame.get("metrics") or []
    order_by = frame.get("order_by") or []
    limit = frame.get("limit")

    contract.required_metrics = [str(m).strip() for m in metrics if str(m).strip()]
    contract.required_order = [str(o).strip() for o in order_by if str(o).strip()]
    if isinstance(limit, int) and limit > 0:
        contract.limit = limit

    # Harvest filter terms and numeric literals from the intent frame filters.
    filters = frame.get("filters") or []
    for flt in filters:
        text = str(flt or "").lower()
        # Tokens: keep moderately long keywords to reduce false positives.
        for token in re.findall(r"[a-z0-9_]+", text):
            if token.isdigit():
                contract.required_numeric_literals.add(token)
            elif len(token) >= 4:
                contract.required_filter_terms.add(token)
        # Numeric literals that may include decimals.
        for num in re.findall(r"\b\d+(?:\.\d+)?\b", text):
            contract.required_numeric_literals.add(num)
        # Symbolic comparisons (>, >=, <, <=, =).
        for op, num in re.findall(r"(>=|<=|>|<|=)\s*(\d+(?:\.\d+)?)", text):
            contract.required_numeric_comparisons.add((num, op))
        # Textual comparisons.
        for num in re.findall(r"(?:at least|no less than|minimum of)\s+(\d+(?:\.\d+)?)", text):
            contract.required_numeric_comparisons.add((num, "at least"))
        for num in re.findall(r"(?:more than|over|above|exceeds?)\s+(\d+(?:\.\d+)?)", text):
            contract.required_numeric_comparisons.add((num, "more than"))
        for num in re.findall(r"(?:less than|under|below|fewer than)\s+(\d+(?:\.\d+)?)", text):
            contract.required_numeric_comparisons.add((num, "less than"))
        for num in re.findall(r"(?:at most|no more than|max(?:imum)? of)\s+(\d+(?:\.\d+)?)", text):
            contract.required_numeric_comparisons.add((num, "at most"))

    # Distinct role expectations: if multiple relationships connect the same labels
    # but with different rel names, encourage separate aliases downstream.
    rel_by_pair = {}
    for (src, rel, dst) in contract.required_edges:
        rel_by_pair.setdefault((src, dst), set()).add(rel)
    for (src, dst), rels in rel_by_pair.items():
        if len(rels) > 1:
            rel_list = sorted(rels)
            for i, rel_a in enumerate(rel_list):
                for rel_b in rel_list[i + 1 :]:
                    contract.required_distinct_roles.add((src, rel_a, rel_b, dst))

    return contract


def _has_aggregate(tokens: Iterable[str]) -> bool:
    agg_funcs = ("count(", "sum(", "avg(", "min(", "max(", "collect(")
    return any(any(func in t.lower() for func in agg_funcs) for t in tokens)


def _has_ratio(tokens: Iterable[str]) -> bool:
    return any("/" in t or "ratio" in t.lower() or "rate" in t.lower() or "share" in t.lower() for t in tokens)


def coverage_violations(contract: RequirementContract, ir, rendered: str) -> List[str]:
    """Check if IR/rendered query satisfies structural and metric expectations."""
    errors: List[str] = []
    if not ir:
        return ["IR missing"]

    label_by_alias: Dict[str, str] = {a: n.label for a, n in ir.nodes.items() if n.label}

    def _canonical(expr: str) -> str:
        """Normalize an expression for comparison (lowercase, resolve aliases → labels)."""
        expr = expr.replace("`", "").strip().lower()

        def _swap_alias(match: re.Match) -> str:
            alias = match.group("alias")
            prop = match.group("prop")
            label = label_by_alias.get(alias, alias)
            return f"{label.lower()}.{prop.lower()}"

        expr = re.sub(r"(?P<alias>[a-z_][a-z0-9_]*)\.(?P<prop>[a-z0-9_]+)", _swap_alias, expr)
        expr = re.sub(r"\s+", " ", expr)
        return expr

    alias_def_pattern = re.compile(r"(.+?)\s+AS\s+([A-Za-z_][A-Za-z0-9_]*)$", re.IGNORECASE)

    def _alias_map() -> Dict[str, str]:
        """Map alias -> canonical source expression from WITH/RETURN."""
        out: Dict[str, str] = {}
        for item in ir.with_items:
            match = alias_def_pattern.match(item.strip())
            if match:
                src, alias = match.group(1).strip(), match.group(2).strip()
                out[alias.lower()] = _canonical(src)
        for r in ir.returns:
            if r.alias:
                out[r.alias.lower()] = _canonical(r.expr)
        return out

    # Node / label coverage
    for label in contract.required_labels:
        if label not in label_by_alias.values():
            errors.append(f"missing required label {label}")

    # Edge coverage
    for (src_label, rel, dst_label) in contract.required_edges:
        present = False
        for e in ir.edges:
            l_label = label_by_alias.get(e.left_alias)
            r_label = label_by_alias.get(e.right_alias)
            if l_label == src_label and r_label == dst_label and e.rel == rel:
                present = True
                break
        if not present:
            errors.append(f"missing required edge {src_label}-[:{rel}]->{dst_label}")

    # Role separation: ensure distinct aliases when multiple rels connect the same labels.
    if contract.required_distinct_roles:
        edges_by_rel: Dict[Tuple[str, str, str], List[Tuple[str, str]]] = {}
        for e in ir.edges:
            l_label = label_by_alias.get(e.left_alias)
            r_label = label_by_alias.get(e.right_alias)
            if not (l_label and r_label):
                continue
            edges_by_rel.setdefault((l_label, e.rel, r_label), []).append((e.left_alias, e.right_alias))
        for (src, rel_a, rel_b, dst) in contract.required_distinct_roles:
            a_aliases = edges_by_rel.get((src, rel_a, dst)) or []
            b_aliases = edges_by_rel.get((src, rel_b, dst)) or []
            if not a_aliases or not b_aliases:
                continue
            for _la, right_a in a_aliases:
                for _lb, right_b in b_aliases:
                    if right_a == right_b:
                        errors.append(
                            f"distinct roles {rel_a}/{rel_b} for {src}->{dst} reuse alias {right_a}; use separate aliases"
                        )
                        break

    # Bidirectional value coverage: ensure both sides of a "between A and B" mention appear.
    if contract.required_bidirectional_values:
        rendered_lower = rendered.lower()
        for left, right in contract.required_bidirectional_values:
            if left not in rendered_lower or right not in rendered_lower:
                errors.append(f"missing symmetric coverage for values '{left}' and '{right}'")

    # Filter token coverage: enforce that salient filter terms and literals survive generation.
    if contract.required_filter_terms:
        rendered_lower = rendered.lower()
        for term in contract.required_filter_terms:
            if term not in rendered_lower:
                errors.append(f"missing required filter term '{term}'")
    if contract.required_numeric_literals:
        rendered_lower = rendered.lower()
        for literal in contract.required_numeric_literals:
            if literal not in rendered_lower:
                errors.append(f"missing required numeric literal '{literal}'")
    if contract.required_numeric_comparisons:
        rendered_lower = rendered.lower()
        for literal, comparator in contract.required_numeric_comparisons:
            missing = False
            if literal not in rendered_lower:
                missing = True
            else:
                if comparator in {">", "<", ">=", "<=", "="}:
                    if comparator not in rendered:
                        missing = True
                else:
                    if comparator not in rendered_lower:
                        missing = True
            if missing:
                errors.append(f"missing comparator '{comparator}' for literal '{literal}'")

    # Zero/absence constraints: require explicit zero-handling when the NL indicated "no/zero X".
    if contract.required_zero_terms:
        expr_blob_lower = " ".join(
            ir.with_items + ir.with_filters + [r.expr for r in ir.returns] + [f"{o.expr}" for o in ir.order_by]
        ).lower()
        # Include rendered text as a fallback when parsing drops the clause.
        expr_blob_lower = f"{expr_blob_lower} {rendered.lower()}"
        for term in contract.required_zero_terms:
            if term not in expr_blob_lower:
                errors.append(f"missing explicit zero-handling for term '{term}'")
                continue
            if not re.search(
                rf"(=\s*0|<=\s*0|\bcount\([^)]*{re.escape(term)}[^)]*\)\s*=\s*0|not\s+exists)",
                expr_blob_lower,
            ):
                errors.append(f"term '{term}' lacks zero/absence constraint")

    # Property coverage
    def _exprs_with_props() -> List[str]:
        exprs: List[str] = []
        exprs.extend([f"{f.alias}.{f.prop}" for f in ir.filters])
        exprs.extend([r.expr for r in ir.returns])
        exprs.extend(ir.with_items)
        exprs.extend(ir.with_filters)
        return exprs

    expr_blob = " ".join(_exprs_with_props())
    for (label, prop) in contract.required_properties:
        if f".{prop}" not in expr_blob:
            errors.append(f"missing required property {label}.{prop}")

    # Metric coverage
    if contract.required_metrics:
        tokens = ir.with_items + [r.expr for r in ir.returns]
        if not _has_aggregate(tokens):
            errors.append("required metrics present but no aggregates defined")
        ratio_needed = any(
            any(term in m.lower() for term in ["rate", "share", "ratio", "percent", "%"]) for m in contract.required_metrics
        )
        if ratio_needed and not _has_ratio(tokens):
            errors.append("ratio/rate metric expected but no ratio-like expression found")

    # Order coverage
    if contract.required_order and not ir.order_by:
        errors.append("order_by required but ORDER BY missing")
    if contract.required_order and ir.order_by:
        alias_to_expr = _alias_map()
        order_exprs = {o.expr.lower(): _canonical(o.expr) for o in ir.order_by}

        def _covers(expected_expr: str) -> bool:
            canonical_expected = _canonical(expected_expr)
            if canonical_expected in order_exprs.values():
                return True
            for raw, _canonical_order in order_exprs.items():
                mapped = alias_to_expr.get(raw)
                if mapped and mapped == canonical_expected:
                    return True
            return False

        for expected in contract.required_order:
            exp_expr = expected.split()[0]
            if not _covers(exp_expr):
                errors.append(f"missing required order key {exp_expr.lower()}")

    # Heuristic type sanity: flag aggregates over temporal-looking fields.
    temporal_suffixes = ("_at", "date", "time")
    agg_patterns = ("avg(", "sum(")

    def _is_temporal(expr: str) -> bool:
        lowered = expr.lower()
        return any(suf in lowered for suf in temporal_suffixes)

    for expr in ir.with_items + [r.expr for r in ir.returns]:
        if any(pat in expr.lower() for pat in agg_patterns) and _is_temporal(expr):
            errors.append(f"suspicious aggregate over temporal field: {expr}")

    # Limit coverage
    if contract.limit is not None and (ir.limit is None or ir.limit > contract.limit):
        errors.append(f"limit should be <= {contract.limit}")

    return sorted(set(errors))


__all__ = ["RequirementContract", "build_contract", "coverage_violations", "contract_view"]


def contract_view(contract: RequirementContract) -> Dict[str, object]:
    """JSON-safe projection of a RequirementContract."""
    return {
        "required_labels": sorted(contract.required_labels),
        "required_edges": sorted([list(e) for e in contract.required_edges]),
        "required_properties": sorted([list(p) for p in contract.required_properties]),
        "required_metrics": contract.required_metrics,
        "required_order": contract.required_order,
        "limit": contract.limit,
        "required_distinct_roles": sorted([list(r) for r in contract.required_distinct_roles]),
        "required_zero_terms": sorted(contract.required_zero_terms),
        "required_bidirectional_values": sorted([list(v) for v in contract.required_bidirectional_values]),
        "required_filter_terms": sorted(contract.required_filter_terms),
        "required_numeric_literals": sorted(contract.required_numeric_literals),
        "required_numeric_comparisons": sorted([list(c) for c in contract.required_numeric_comparisons]),
    }


