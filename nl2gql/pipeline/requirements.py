from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Optional, Set, Tuple
import re

from .schema_graph import SchemaGraph


@dataclass
class RoleConstraint:
    label: Optional[str] = None
    distinct_from: List[str] = field(default_factory=list)


@dataclass
class RequirementContract:
    required_labels: Set[str] = field(default_factory=set)
    required_edges: Set[Tuple[str, str, str]] = field(default_factory=set)  # (src_label, rel, dst_label)
    required_order: List[str] = field(default_factory=list)
    required_outputs: List[str] = field(default_factory=list)
    limit: Optional[int] = None
    # Roles that should be represented by distinct aliases (e.g., ORIGIN vs DESTINATION).
    required_distinct_roles: Set[Tuple[str, str, str, str]] = field(default_factory=set)
    # Semantic roles (tagged) that should map to dedicated aliases with optional distinctness.
    role_constraints: Dict[str, RoleConstraint] = field(default_factory=dict)
    # Concrete alias bindings resolved during enforcement (role -> alias).
    role_aliases: Dict[str, str] = field(default_factory=dict)
    role_distinct_filters: Set[Tuple[str, str]] = field(default_factory=set)


def _canonicalize_function_name(name: str) -> str:
    """Map function names to a canonical lowercase form."""
    name = name.lower()
    synonyms = {
        "average": "avg",
        "mean": "avg",
    }
    return synonyms.get(name, name)


def _canonicalize_expr(expr: str) -> str:
    """
    Canonicalize an expression for contract storage:
    - strip backticks
    - lowercase function names with synonym mapping
    - collapse whitespace
    """
    expr = expr.replace("`", "").strip()

    def _normalize_func(match: re.Match) -> str:
        func = _canonicalize_function_name(match.group("func"))
        return f"{func}("

    expr = re.sub(r"(?P<func>[A-Za-z_][A-Za-z0-9_]*)\s*\(", _normalize_func, expr)
    expr = re.sub(r"\s+", " ", expr).strip()
    return expr


def build_contract(nl: str, pre, guidance, graph: SchemaGraph) -> RequirementContract:
    """Derive structural and metric expectations from intent+linking output (schema-aware)."""
    contract = RequirementContract()

    # Token sets for grounding: only tokens present in NL or schema become "hard" constraints.
    lowered_nl = pre.normalized_nl.lower() if hasattr(pre, "normalized_nl") else nl.lower()

    links = guidance.links or {}
    node_links = links.get("node_links") or []
    rel_links = links.get("rel_links") or []
    canonical_aliases = links.get("canonical_aliases") or {}

    role_constraints: Dict[str, RoleConstraint] = {}
    roles_by_label: Dict[str, List[str]] = {}

    def _register_role(role_name: str, label: Optional[str]) -> None:
        if not role_name:
            return
        rc = role_constraints.setdefault(role_name, RoleConstraint())
        if label and not rc.label:
            rc.label = label
        if label:
            if role_name not in roles_by_label.get(label, []):
                roles_by_label.setdefault(label, []).append(role_name)

    alias_to_label: Dict[str, str] = {}
    for nl in node_links:
        alias, label = nl.get("alias"), nl.get("label")
        if alias and label and graph.has_node(label):
            alias_to_label[alias] = label
            contract.required_labels.add(label)
            _register_role(alias, label)
        elif alias and label:
            # Keep a role tag even if label not validated yet; it can be filled by closest match later.
            _register_role(alias, label if graph.has_node(label) else None)
        elif alias and alias in canonical_aliases:
            _register_role(alias, canonical_aliases.get(alias))

    for rl in rel_links:
        left, rel, right = rl.get("left_alias"), rl.get("rel"), rl.get("right_alias")
        if not (left and rel and right):
            continue
        left_label = alias_to_label.get(left)
        right_label = alias_to_label.get(right)
        if left_label and right_label and graph.edge_exists(left_label, rel, right_label):
            contract.required_edges.add((left_label, rel, right_label))

    # Detect NL hints that imply distinct roles for the same label (e.g., "different city").
    distinct_tokens = ("different", "other", "another", "separate", "elsewhere")
    distinct_labels: Set[str] = set()
    for label in graph.nodes:
        lbl = label.lower()
        for tok in distinct_tokens:
            if re.search(rf"\b{tok}\s+{re.escape(lbl)}s?\b", lowered_nl):
                distinct_labels.add(label)
                break
    # If NL implies distinctness for a label but we only have a single role, create an alternate role.
    role_seq: Dict[str, int] = {label: len(roles) for label, roles in roles_by_label.items()}
    for label in distinct_labels:
        existing = roles_by_label.get(label, [])
        if len(existing) >= 2:
            continue
        role_seq[label] = role_seq.get(label, 0) + 1
        alt_role = f"{label.lower()}_distinct_{role_seq[label]}"
        _register_role(alt_role, label)

    frame = guidance.frame or {}
    targets = frame.get("targets") or []
    metrics = frame.get("metrics") or []
    order_by = frame.get("order_by") or []
    limit = frame.get("limit")

    contract.required_outputs = [
        _canonicalize_expr(str(t).strip()) for t in (targets + metrics) if str(t).strip()
    ]
    contract.required_order = [_canonicalize_expr(str(o).strip()) for o in order_by if str(o).strip()]
    if isinstance(limit, int) and limit > 0:
        contract.limit = limit

    # Populate role constraints and distinctness expectations.
    for label, roles in roles_by_label.items():
        if len(roles) < 2:
            continue
        for i, role_a in enumerate(roles):
            rc_a = role_constraints.get(role_a)
            if not rc_a:
                continue
            for role_b in roles[i + 1 :]:
                rc_b = role_constraints.get(role_b)
                if not rc_b:
                    continue
                # Default to distinct aliases when multiple roles share a label; NL cues reinforce this.
                if role_b not in rc_a.distinct_from:
                    rc_a.distinct_from.append(role_b)
                if role_a not in rc_b.distinct_from:
                    rc_b.distinct_from.append(role_a)

    contract.role_constraints = role_constraints

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


def coverage_violations(contract: RequirementContract, ir, rendered: str) -> List[str]:
    """Check if IR/rendered query satisfies structural expectations."""
    del rendered
    errors: List[str] = []
    if not ir:
        return ["IR missing"]

    label_by_alias: Dict[str, str] = {a: n.label for a, n in ir.nodes.items() if n.label}
    alias_def_pattern = re.compile(r"(.+?)\s+AS\s+([A-Za-z_][A-Za-z0-9_]*)$", re.IGNORECASE)

    def _alias_sources() -> Dict[str, str]:
        out: Dict[str, str] = {}
        for item in ir.with_items:
            match = alias_def_pattern.match(item.strip())
            if match:
                src, alias = match.group(1).strip(), match.group(2).strip()
                out[alias.lower()] = src
        for r in ir.returns:
            if r.alias:
                out[r.alias.lower()] = r.expr
        return out

    def _canonicalize_funcs(expr: str) -> str:
        def _normalize_func(match: re.Match) -> str:
            func = _canonicalize_function_name(match.group("func"))
            return f"{func}("

        return re.sub(r"(?P<func>[A-Za-z_][A-Za-z0-9_]*)\s*\(", _normalize_func, expr)

    def _normalize(expr: str, alias_sources: Dict[str, str]) -> str:
        expr = expr.replace("`", "").strip()
        # Drop trailing alias definitions for comparison.
        expr = re.split(r"\s+AS\s+", expr, flags=re.IGNORECASE)[0].strip()
        for _ in range(5):
            src = alias_sources.get(expr.lower())
            if not src:
                break
            expr = src.strip()
        expr = expr.lower()
        expr = re.sub(r"\bdistinct\s+", "", expr)

        def _swap_alias_prop(match: re.Match) -> str:
            alias = match.group("alias")
            prop = match.group("prop")
            label = label_by_alias.get(alias, alias).lower()
            return f"{label}.{prop}"

        expr = re.sub(r"(?P<alias>[a-z_][a-z0-9_]*)\.(?P<prop>[a-z0-9_]+)", _swap_alias_prop, expr)

        def _swap_func_alias(match: re.Match) -> str:
            func = _canonicalize_function_name(match.group("func"))
            alias = match.group("alias")
            label = label_by_alias.get(alias, alias).lower()
            return f"{func}({label})"

        expr = re.sub(
            r"(?P<func>[A-Za-z_][A-Za-z0-9_]*)\(\s*(?P<alias>[a-z_][a-z0-9_]*)\s*\)",
            _swap_func_alias,
            expr,
        )
        expr = _canonicalize_funcs(expr)
        expr = re.sub(r"\s+", " ", expr).strip()
        return expr

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

    if contract.role_distinct_filters:
        present_pairs: Set[Tuple[str, str]] = set()
        for flt in ir.filters:
            if flt.op != "<>":
                continue
            other_alias = None
            if isinstance(flt.value, dict):
                other_alias = flt.value.get("ref_alias")
            if not other_alias:
                continue
            pair = tuple(sorted((flt.alias, other_alias)))
            present_pairs.add(pair)
        for pair in contract.role_distinct_filters:
            if pair not in present_pairs:
                errors.append(f"missing distinct filter between aliases {pair[0]} and {pair[1]}")

    # Output coverage: ensure required outputs are returned.
    if contract.required_outputs:
        alias_sources = _alias_sources()
        normalized_returns = {_normalize(r.expr, alias_sources) for r in ir.returns}
        for expected in contract.required_outputs:
            expected = str(expected).strip()
            if not expected:
                continue
            if _normalize(expected, alias_sources) not in normalized_returns:
                errors.append(f"missing required output {expected.lower()}")

    # Order coverage
    if contract.required_order and not ir.order_by:
        errors.append("order_by required but ORDER BY missing")
    if contract.required_order and ir.order_by:
        alias_sources = _alias_sources()
        normalized_order = {_normalize(o.expr, alias_sources) for o in ir.order_by}

        for expected in contract.required_order:
            exp_expr = expected.split()[0]
            if _normalize(exp_expr, alias_sources) not in normalized_order:
                errors.append(f"missing required order key {exp_expr.lower()}")

    # Limit coverage
    if contract.limit is not None and (ir.limit is None or ir.limit > contract.limit):
        errors.append(f"limit should be <= {contract.limit}")

    return sorted(set(errors))


__all__ = [
    "RequirementContract",
    "RoleConstraint",
    "build_contract",
    "coverage_violations",
    "contract_view",
    "resolve_required_output_forms",
]


def contract_view(contract: RequirementContract) -> Dict[str, object]:
    """JSON-safe projection of a RequirementContract."""
    return {
        "required_labels": sorted(contract.required_labels),
        "required_edges": sorted([list(e) for e in contract.required_edges]),
        "required_outputs": contract.required_outputs,
        "required_order": contract.required_order,
        "limit": contract.limit,
        "required_distinct_roles": sorted([list(r) for r in contract.required_distinct_roles]),
        "role_constraints": {
            role: {
                "label": rc.label,
                "distinct_from": sorted(set(rc.distinct_from)),
            }
            for role, rc in sorted(contract.role_constraints.items())
        },
        "role_aliases": {role: alias for role, alias in sorted(contract.role_aliases.items())},
        "role_distinct_filters": sorted([list(pair) for pair in contract.role_distinct_filters]),
    }


def resolve_required_output_forms(contract: RequirementContract, ir: Optional["ISOQueryIR"] = None) -> Dict[str, List[str]]:
    """
    Provide both the original NL-derived outputs and alias-level equivalents based on
    resolved role aliases / IR labels so downstream prompts can reference concrete expressions.
    """
    label_to_alias: Dict[str, str] = {}
    for role, alias in (contract.role_aliases or {}).items():
        rc = (contract.role_constraints or {}).get(role)
        if rc and rc.label and alias:
            label_to_alias.setdefault(rc.label, alias)

    if ir:
        for alias, node in ir.nodes.items():
            if node.label and alias:
                label_to_alias.setdefault(node.label, alias)

    alias_forms: List[str] = []
    canonical_forms: List[str] = []
    canonical_nodistinct_forms: List[str] = []

    for expr in contract.required_outputs:
        alias_expr = expr
        for label, alias in label_to_alias.items():
            pattern_dot = re.compile(rf"(?i)\b{re.escape(label)}\.")
            alias_expr = pattern_dot.sub(f"{alias}.", alias_expr)
            pattern_word = re.compile(rf"(?i)\b{re.escape(label)}\b")
            alias_expr = pattern_word.sub(alias, alias_expr)
        alias_forms.append(alias_expr)
        canonical = _canonicalize_expr(alias_expr)
        canonical_forms.append(canonical)
        canonical_nodistinct_forms.append(re.sub(r"(?i)\bdistinct\s+", "", canonical).strip())

    return {
        "original": list(contract.required_outputs),
        "alias": alias_forms,
        "alias_canonical": canonical_forms,
        "alias_canonical_nodistinct": canonical_nodistinct_forms,
    }
