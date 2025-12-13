from __future__ import annotations

import inspect
import json
import re
import time
from collections import OrderedDict, defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Tuple

from .config import DEFAULT_OPENAI_MODEL_FIX
from .generator import CandidateQuery, QueryGenerator
from .intent_linker import IntentLinkGuidance, links_to_hints
from .ir import IRFilter, IREdge, IRNode, IROrder, IRReturn, ISOQueryIR
from .openai_client import chat_complete, clean_block
from .preprocess import PreprocessResult, Preprocessor
from .requirements import RequirementContract, RoleConstraint, build_contract, contract_view, coverage_violations
from .run_logger import RunLogger
from .runner import GraphLiteRunner, SyntaxResult
from .schema_graph import SchemaEdge, SchemaGraph
from .structural_validator import validate_structure
from .ui import Spinner
from .validators import LogicValidator, SchemaGroundingValidator


@dataclass
class ValidationBundle:
    ir: Optional[ISOQueryIR]
    parse_errors: List[str]
    structural_errors: List[str]
    schema_errors: List[str]
    coverage_errors: List[str]
    syntax_result: SyntaxResult
    logic_valid: bool
    logic_reason: Optional[str]
    repaired: bool = False
    query_text: str = ""
    fix_applied: bool = False
    fix_details: Optional[str] = None
    fixes: List[Dict[str, Any]] = field(default_factory=list)


class PipelineFailure(Exception):
    def __init__(self, message: str, timeline: List[Dict[str, Any]], failures: List[str]) -> None:
        super().__init__(message)
        self.timeline = timeline
        self.failures = failures


class Refiner:
    def __init__(
        self,
        graph: SchemaGraph,
        generator: QueryGenerator,
        *,
        logic_validator: Optional[LogicValidator] = None,
        runner: Optional[GraphLiteRunner] = None,
        db_path: Optional[str] = None,
        max_loops: int = 2,
    ) -> None:
        self.graph = graph
        self.generator = generator
        self.logic_validator = logic_validator or LogicValidator()
        self.runner = runner or GraphLiteRunner(db_path=db_path)
        self.max_loops = max_loops

    def _fresh_alias(self, base: str, existing: set) -> str:
        candidate = re.sub(r"[^a-z0-9_]", "", base.lower()) or "n"
        if candidate[0].isdigit():
            candidate = f"n{candidate}"
        while candidate in existing:
            candidate += "1"
        return candidate

    def _enforce_contract_structure(self, ir: ISOQueryIR, contract: RequirementContract) -> None:
        """Add missing nodes/edges/properties required by the contract in a schema-agnostic way."""
        existing_aliases = set(ir.nodes.keys())

        # Track role-tagged expectations to keep aliases distinct when requested.
        role_aliases: Dict[str, str] = {}
        role_distinct: Dict[str, Set[str]] = defaultdict(set)
        roles_by_label: Dict[str, List[str]] = defaultdict(list)
        for role, rc in (contract.role_constraints or {}).items():
            if not rc:
                continue
            if rc.label:
                if role not in roles_by_label[rc.label]:
                    roles_by_label[rc.label].append(role)
            for other in rc.distinct_from:
                if other:
                    role_distinct[role].add(other)
                    role_distinct[other].add(role)

        # Build label -> aliases map for reuse.
        label_to_aliases: Dict[str, List[str]] = defaultdict(list)
        for alias, node in ir.nodes.items():
            if node.label:
                label_to_aliases[node.label].append(alias)

        def ensure_role_alias(role: str, rc: RoleConstraint) -> str:
            if role in role_aliases:
                alias_existing = role_aliases[role]
                node = ir.nodes.get(alias_existing)
                if node and rc.label and node.label is None:
                    node.label = rc.label
                return alias_existing

            avoid_aliases = {role_aliases[o] for o in role_distinct.get(role, set()) if o in role_aliases}
            preferred_label = rc.label
            preferred_alias = role
            alias_candidate: Optional[str] = None

            if preferred_alias in ir.nodes and preferred_alias not in avoid_aliases:
                node = ir.nodes[preferred_alias]
                if preferred_label and node.label is None:
                    node.label = preferred_label
                if not preferred_label or node.label is None or node.label == preferred_label:
                    alias_candidate = preferred_alias

            if alias_candidate is None and preferred_label:
                for cand in label_to_aliases.get(preferred_label, []):
                    if cand not in avoid_aliases:
                        alias_candidate = cand
                        break

            if alias_candidate is None:
                base_seed = preferred_label[:2] if preferred_label else re.sub(r"[^a-z0-9_]", "", preferred_alias.lower())[:2]
                base = base_seed or "n"
                alias_candidate = self._fresh_alias(base, existing_aliases | avoid_aliases)
                ir.nodes[alias_candidate] = IRNode(alias=alias_candidate, label=preferred_label)
                existing_aliases.add(alias_candidate)
                if preferred_label:
                    label_to_aliases[preferred_label].append(alias_candidate)

            role_aliases[role] = alias_candidate
            return alias_candidate

        role_cursor: Dict[str, int] = defaultdict(int)

        def get_alias_for_label(label: str, avoid: Optional[set] = None, role_hint: Optional[str] = None) -> str:
            avoid = avoid or set()
            if role_hint and role_hint in (contract.role_constraints or {}):
                rc = contract.role_constraints[role_hint]
                if rc.label is None:
                    rc.label = label
                    if role_hint not in roles_by_label[label]:
                        roles_by_label[label].append(role_hint)
                alias_for_role = ensure_role_alias(role_hint, rc)
                if alias_for_role in avoid:
                    alias_for_role = self._fresh_alias(label[:2], existing_aliases | avoid)
                    existing_aliases.add(alias_for_role)
                    ir.nodes[alias_for_role] = IRNode(alias=alias_for_role, label=label)
                    role_aliases[role_hint] = alias_for_role
                    label_to_aliases[label].append(alias_for_role)
                return alias_for_role

            role_list = roles_by_label.get(label, [])
            if role_list:
                start_idx = role_cursor[label] % len(role_list)
                for offset in range(len(role_list)):
                    role = role_list[(start_idx + offset) % len(role_list)]
                    rc = contract.role_constraints.get(role)
                    if not rc:
                        continue
                    alias_for_role = ensure_role_alias(role, rc)
                    if alias_for_role in avoid:
                        continue
                    role_cursor[label] = (start_idx + offset + 1) % len(role_list)
                    return alias_for_role

            for cand in label_to_aliases.get(label, []):
                if cand not in avoid:
                    return cand
            alias = self._fresh_alias(label[:2], existing_aliases | avoid)
            existing_aliases.add(alias)
            ir.nodes[alias] = IRNode(alias=alias, label=label)
            label_to_aliases[label].append(alias)
            return alias

        # Pre-allocate aliases for roles so downstream nodes/edges reuse them.
        for role, rc in (contract.role_constraints or {}).items():
            if not rc.label:
                continue
            ensure_role_alias(role, rc)

        # Ensure required edges exist.
        for src_label, rel, dst_label in contract.required_edges:
            src_alias = get_alias_for_label(src_label)
            dst_alias = get_alias_for_label(dst_label, avoid={src_alias})
            if not any(
                e.left_alias == src_alias and e.right_alias == dst_alias and e.rel == rel for e in ir.edges
            ):
                ir.edges.append(IREdge(left_alias=src_alias, rel=rel, right_alias=dst_alias))

        # Enforce distinct role aliases via inequality filters.
        if role_distinct:
            existing_filter_keys = {(f.alias, f.prop, f.op, str(f.value)) for f in ir.filters}
            distinct_pairs: set[frozenset[str]] = set()
            for role, others in role_distinct.items():
                rc = (contract.role_constraints or {}).get(role)
                if rc and role not in role_aliases and rc.label:
                    ensure_role_alias(role, rc)
                alias_a = role_aliases.get(role)
                if not alias_a:
                    continue
                for other in others:
                    rc_other = (contract.role_constraints or {}).get(other)
                    if rc_other and other not in role_aliases and rc_other.label:
                        ensure_role_alias(other, rc_other)
                    alias_b = role_aliases.get(other)
                    if not alias_b or alias_a == alias_b:
                        continue
                    pair_key = frozenset({alias_a, alias_b})
                    if pair_key in distinct_pairs:
                        continue
                    distinct_pairs.add(pair_key)
                    flt_value = {"ref_alias": alias_b, "ref_property": "id"}
                    key = (alias_a, "id", "<>", str(flt_value))
                    if key in existing_filter_keys:
                        continue
                    ir.filters.append(IRFilter(alias=alias_a, prop="id", op="<>", value=flt_value))
                    existing_filter_keys.add(key)

        # Ensure required properties appear somewhere (at least in RETURN) to keep them visible.
        agg_pattern = re.compile(r"\b(count|sum|avg|min|max|collect)\s*\(", re.IGNORECASE)
        expr_bag = (
            list(ir.with_items)
            + list(ir.with_filters)
            + [r.expr for r in ir.returns]
            + [o.expr for o in ir.order_by]
            + [f"{flt.alias}.{flt.prop}" for flt in ir.filters]
        )

        def _prop_already_used(label: str, prop: str) -> bool:
            aliases_for_label = label_to_aliases.get(label, [])
            for expr in expr_bag:
                for alias in aliases_for_label:
                    if re.search(rf"\b{re.escape(alias)}\.{re.escape(prop)}\b", expr):
                        return True
            return False

        has_agg_context = any(agg_pattern.search(expr) for expr in expr_bag)

        for label, prop in contract.required_properties:
            if _prop_already_used(label, prop):
                continue
            alias = get_alias_for_label(label)
            expr = f"{alias}.{prop}"
            # In aggregate-heavy queries, avoid injecting raw properties that would
            # force an unintended grouping explosion; treat them as optional hints.
            if has_agg_context:
                continue
            ir.returns.append(IRReturn(expr=expr, alias=None))
            expr_bag.append(expr)

        # Seed ORDER BY when the contract requires it but the query omitted it.
        if contract.required_order and not ir.order_by:
            for item in contract.required_order:
                parts = item.split()
                if not parts:
                    continue
                expr = parts[0]
                direction = parts[1] if len(parts) > 1 else "DESC"
                ir.order_by.append(IROrder(expr=expr, direction=direction.upper()))

        # Respect limit if stricter than current.
        if contract.limit is not None:
            if ir.limit is None or ir.limit > contract.limit:
                ir.limit = contract.limit

    def _ensure_grouping(self, ir: ISOQueryIR) -> None:
        """
        If aggregates are present alongside non-aggregated expressions, ensure WITH/RETURN
        are consistent:
        - preserve grouping keys explicitly in WITH
        - emit single-pass aliases (no alias-of-alias)
        - rewrite ORDER BY to the chosen aliases
        """
        agg_pattern = re.compile(r"\b(count|sum|avg|average|min|max|collect)\s*\(", re.IGNORECASE)
        has_agg = any(agg_pattern.search(r.expr) for r in ir.returns) or any(
            agg_pattern.search(w) for w in ir.with_items
        )
        if not has_agg:
            return

        non_agg_returns: List[IRReturn] = [r for r in ir.returns if not agg_pattern.search(r.expr)]
        if not non_agg_returns:
            return

        with_map: "OrderedDict[str, str]" = OrderedDict()
        for item in ir.with_items:
            if not item:
                continue
            parts = re.split(r"\s+AS\s+", item, flags=re.IGNORECASE)
            expr = parts[0].strip()
            alias = parts[1].strip() if len(parts) == 2 else expr
            if alias not in with_map:
                with_map[alias] = expr

        def ensure_alias(expr: str, preferred: Optional[str] = None) -> str:
            # Reuse existing alias if it already represents this expression.
            for alias, target in with_map.items():
                if expr == alias or expr == target:
                    return alias
            candidate = preferred
            if candidate and candidate in with_map and with_map[candidate] != expr:
                candidate = None
            alias = candidate or self._fresh_alias(expr.replace(".", "_")[:8] or "expr", set(with_map.keys()))
            with_map.setdefault(alias, expr)
            return alias

        alias_map: Dict[str, str] = {}
        for ret in ir.returns:
            alias = ensure_alias(ret.expr, preferred=ret.alias)
            alias_map[ret.expr] = alias

        def _with_item(alias: str, expr: str) -> str:
            return expr if alias == expr else f"{expr} AS {alias}"

        ir.with_items = [_with_item(alias, expr) for alias, expr in with_map.items()]
        ir.returns = [IRReturn(expr=alias_map.get(r.expr, r.expr)) for r in ir.returns]

        def _resolve(expr: str) -> str:
            for alias, target in with_map.items():
                if expr == alias or expr == target:
                    return alias
            return alias_map.get(expr, expr)

        ir.order_by = [IROrder(expr=_resolve(o.expr), direction=o.direction) for o in ir.order_by]

    def _persist_debug(self, run_logger: Optional[RunLogger], task_label: str, payload: Dict[str, Any]) -> None:
        if not run_logger:
            return
        run_logger.log_debug({"task": task_label, **payload})

    def _deterministic_schema_repair(self, ir: ISOQueryIR) -> bool:
        """Auto-fix common schema errors deterministically (e.g., flip edge directions)."""
        changed = False
        for edge in list(ir.edges):
            # Check if edge exists in schema
            left_label = ir.nodes.get(edge.left_alias, IRNode(alias=edge.left_alias)).label
            right_label = ir.nodes.get(edge.right_alias, IRNode(alias=edge.right_alias)).label
            if not (left_label and right_label):
                continue
            if self.graph.edge_exists(left_label, edge.rel, right_label):
                continue  # Already correct
            # Check reverse
            if self.graph.edge_exists(right_label, edge.rel, left_label):
                # Flip the edge
                edge.left_alias, edge.right_alias = edge.right_alias, edge.left_alias
                changed = True
                # Optionally log: print(f"Auto-flipped edge {left_label}-[:{edge.rel}]->{right_label} to {right_label}-[:{edge.rel}]->{left_label}")
        return changed

    def _repair_ir_schema(
        self,
        ir: ISOQueryIR,
        *,
        alias_label_hints: Optional[Dict[str, str]] = None,
        rel_hints: Optional[List[Tuple[str, str, str]]] = None,
        label_hints: Optional[List[Tuple[str, str, str]]] = None,
    ) -> bool:
        changed = False
        alias_label_hints = alias_label_hints or {}
        rel_hints = rel_hints or []
        label_hints = label_hints or []
        schema_by_rel: Dict[str, List[SchemaEdge]] = defaultdict(list)
        for schema_edge in self.graph.edges:
            schema_by_rel[schema_edge.rel].append(schema_edge)

        for alias, label in alias_label_hints.items():
            if not self.graph.has_node(label):
                continue
            node = ir.nodes.setdefault(alias, IRNode(alias=alias))
            if node.label is None:
                node.label = label
                changed = True

        prop_index: Dict[str, set] = defaultdict(set)
        for lbl, node in self.graph.nodes.items():
            for prop in node.properties:
                prop_index[prop].add(lbl)
        for alias, node in ir.nodes.items():
            if node.label:
                continue
            props_for_alias = {flt.prop for flt in ir.filters if flt.alias == alias}
            for ret in ir.returns:
                for alias_ref, prop in re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)", ret.expr):
                    if alias_ref == alias:
                        props_for_alias.add(prop)
            for prop in props_for_alias:
                owners = prop_index.get(prop) or set()
                if len(owners) == 1:
                    node.label = next(iter(owners))
                    changed = True
                    break

        rel_hint_map: Dict[Tuple[str, str], str] = {}
        for left, rel, right in rel_hints:
            if left and right and rel:
                rel_hint_map[(left, right)] = rel

        label_hint_set: set[Tuple[str, str, str]] = set()
        for l_src, l_rel, l_dst in label_hints:
            if l_src and l_rel and l_dst:
                label_hint_set.add((l_src, l_rel, l_dst))

        for edge in ir.edges:
            left_node = ir.nodes.setdefault(edge.left_alias, IRNode(alias=edge.left_alias))
            right_node = ir.nodes.setdefault(edge.right_alias, IRNode(alias=edge.right_alias))
            left_label = left_node.label
            right_label = right_node.label

            hinted_rel = rel_hint_map.get((edge.left_alias, edge.right_alias))
            if hinted_rel and hinted_rel != edge.rel:
                edge.rel = hinted_rel
                changed = True
            hinted_rel_flipped = rel_hint_map.get((edge.right_alias, edge.left_alias))
            if hinted_rel_flipped and hinted_rel_flipped != edge.rel:
                edge.left_alias, edge.right_alias = edge.right_alias, edge.left_alias
                left_node, right_node = right_node, left_node
                left_label, right_label = right_label, left_label
                edge.rel = hinted_rel_flipped
                changed = True

            # Label-level hints to quickly fill missing labels and orient edges.
            for lbl_src, lbl_rel, lbl_dst in label_hint_set:
                if lbl_rel != edge.rel:
                    continue
                if left_label is None and right_label is None:
                    left_node.label = lbl_src
                    right_node.label = lbl_dst
                    left_label, right_label = lbl_src, lbl_dst
                    changed = True
                    break
                if left_label == lbl_dst and right_label is None:
                    right_node.label = lbl_src
                    edge.left_alias, edge.right_alias = edge.right_alias, edge.left_alias
                    left_node, right_node = right_node, left_node
                    left_label, right_label = right_label, left_label
                    changed = True
                    break
                if left_label == lbl_src and right_label is None:
                    right_node.label = lbl_dst
                    right_label = lbl_dst
                    changed = True
                    break
                if right_label == lbl_dst and left_label is None:
                    left_node.label = lbl_src
                    left_label = lbl_src
                    changed = True
                    break
                if right_label == lbl_src and left_label is None:
                    left_node.label = lbl_dst
                    edge.left_alias, edge.right_alias = edge.right_alias, edge.left_alias
                    left_node, right_node = right_node, left_node
                    left_label, right_label = right_label, left_label
                    changed = True
                    break

            if left_label and right_label and any(
                e.src == left_label and e.rel == edge.rel and e.dst == right_label for e in self.graph.edges
            ):
                continue

            for schema_edge in self.graph.edges:
                if schema_edge.rel != edge.rel:
                    continue

                left_matches = left_label is None or left_label == schema_edge.src
                right_matches = right_label is None or right_label == schema_edge.dst
                if left_matches and right_matches:
                    if left_label is None:
                        left_node.label = schema_edge.src
                        left_label = schema_edge.src
                        changed = True
                    if right_label is None:
                        right_node.label = schema_edge.dst
                        right_label = schema_edge.dst
                        changed = True
                    break
                # If one side matches, coerce the other label to the schema edge to reduce mismatched rel targets.
                if left_label == schema_edge.src and right_label and right_label != schema_edge.dst:
                    right_node.label = schema_edge.dst
                    right_label = schema_edge.dst
                    changed = True
                if right_label == schema_edge.dst and left_label and left_label != schema_edge.src:
                    left_node.label = schema_edge.src
                    left_label = schema_edge.src
                    changed = True
            if left_label and right_label and any(
                e.src == right_label and e.rel == edge.rel and e.dst == left_label for e in self.graph.edges
            ):
                edge.left_alias, edge.right_alias = edge.right_alias, edge.left_alias
                changed = True

            # Snap invalid edges to the closest schema edge with the same relationship name.
            candidates = schema_by_rel.get(edge.rel, [])
            if candidates:
                def _score(candidate: SchemaEdge) -> int:
                    score = 0
                    if left_label == candidate.src:
                        score += 3
                    if right_label == candidate.dst:
                        score += 3
                    if left_label == candidate.dst:
                        score += 2
                    if right_label == candidate.src:
                        score += 2
                    if (candidate.src, candidate.rel, candidate.dst) in label_hint_set:
                        score += 4
                    if (candidate.dst, candidate.rel, candidate.src) in label_hint_set:
                        score += 2
                    return score

                best = max(candidates, key=_score)

                # Flip aliases if the reversed direction is a better fit.
                if (left_label == best.dst and right_label == best.src) or (
                    left_label is None and right_label == best.src
                ):
                    edge.left_alias, edge.right_alias = edge.right_alias, edge.left_alias
                    left_node, right_node = right_node, left_node
                    left_label, right_label = right_label, left_label
                    changed = True

                # Align labels to the schema edge, cloning aliases when a self-loop must be split.
                if left_label != best.src:
                    if edge.left_alias == edge.right_alias or (left_label and left_label != best.src):
                        new_alias = self._fresh_alias(best.src[:2], set(ir.nodes.keys()))
                        ir.nodes[new_alias] = IRNode(alias=new_alias, label=best.src)
                        edge.left_alias = new_alias
                        left_node = ir.nodes[new_alias]
                    else:
                        left_node.label = best.src
                    left_label = best.src
                    changed = True

                if right_label != best.dst:
                    if edge.left_alias == edge.right_alias or (right_label and right_label != best.dst):
                        new_alias = self._fresh_alias(best.dst[:2], set(ir.nodes.keys()))
                        ir.nodes[new_alias] = IRNode(alias=new_alias, label=best.dst)
                        edge.right_alias = new_alias
                        right_node = ir.nodes[new_alias]
                    else:
                        right_node.label = best.dst
                    right_label = best.dst
                    changed = True

        # Promote relational diversity when the schema exposes multiple relationship
        # types between the same labels but the candidate repeats only one of them.
        rel_options_by_label_pair: Dict[Tuple[str, str], set[str]] = defaultdict(set)
        for l_src, l_rel, l_dst in label_hint_set:
            rel_options_by_label_pair[(l_src, l_dst)].add(l_rel)
        edge_counts: Dict[Tuple[Optional[str], str, Optional[str]], int] = defaultdict(int)
        for e in ir.edges:
            l_lbl = ir.nodes.get(e.left_alias, IRNode(alias=e.left_alias)).label
            r_lbl = ir.nodes.get(e.right_alias, IRNode(alias=e.right_alias)).label
            edge_counts[(l_lbl, e.rel, r_lbl)] += 1
        for edge in ir.edges:
            l_lbl = ir.nodes.get(edge.left_alias, IRNode(alias=edge.left_alias)).label
            r_lbl = ir.nodes.get(edge.right_alias, IRNode(alias=edge.right_alias)).label
            options = rel_options_by_label_pair.get((l_lbl, r_lbl))
            if not options or len(options) < 2:
                continue
            if edge_counts[(l_lbl, edge.rel, r_lbl)] <= 1:
                continue
            missing = [opt for opt in options if opt != edge.rel]
            if missing:
                edge.rel = missing[0]
                changed = True
                break

        return changed

    def _dedupe_edges(self, ir: ISOQueryIR) -> None:
        seen: set[tuple[str, str, str]] = set()
        unique_edges: list[IREdge] = []
        for e in ir.edges:
            key = (e.left_alias, e.rel, e.right_alias)
            if key in seen:
                continue
            seen.add(key)
            unique_edges.append(e)
        # Secondary dedupe by label triplet to avoid repeating the same semantic edge
        # with different aliases (common when the model mirrors the same edge twice).
        seen_label_triplets: set[Tuple[Optional[str], str, Optional[str]]] = set()
        filtered: List[IREdge] = []
        for e in unique_edges:
            l_label = ir.nodes.get(e.left_alias, IRNode(alias=e.left_alias)).label
            r_label = ir.nodes.get(e.right_alias, IRNode(alias=e.right_alias)).label
            label_key = (l_label, e.rel, r_label)
            if label_key in seen_label_triplets:
                continue
            seen_label_triplets.add(label_key)
            filtered.append(e)
        ir.edges = filtered

    def _prune_returns(self, ir: ISOQueryIR) -> None:
        """Remove return expressions that reference unknown aliases or properties outside the schema."""
        pruned: List[IRReturn] = []
        for ret in ir.returns:
            expr = ret.expr.strip()
            if not expr:
                continue
            # Allow aggregate expressions without deep checks.
            if re.search(r"\b(count|sum|avg|min|max|collect)\s*\(", expr, re.IGNORECASE):
                pruned.append(ret)
                continue
            alias_match = re.match(r"([A-Za-z_][A-Za-z0-9_]*)\\.([A-Za-z_][A-Za-z0-9_]*)", expr)
            if alias_match:
                alias, prop = alias_match.groups()
                node = ir.nodes.get(alias)
                if node and node.label and self.graph.has_property(node.label, prop):
                    pruned.append(ret)
                elif node and node.label is None:
                    pruned.append(ret)
                continue
            # Keep bare aliases that exist.
            if expr in ir.nodes:
                pruned.append(ret)
        if pruned:
            ir.returns = pruned

    def _ensure_order_fields(self, ir: ISOQueryIR) -> None:
        if not ir.order_by:
            return
        return_exprs = {r.expr for r in ir.returns}
        for o in ir.order_by:
            if o.expr not in return_exprs:
                ir.returns.append(IRReturn(expr=o.expr))

    def _prune_unconnected_nodes(self, ir: ISOQueryIR) -> None:
        """Drop nodes that are never referenced by edges, filters, projections, or ordering."""
        referenced: set[str] = set()
        for e in ir.edges:
            referenced.add(e.left_alias)
            referenced.add(e.right_alias)
        for flt in ir.filters:
            referenced.add(flt.alias)
            if isinstance(flt.value, dict):
                ref_alias = flt.value.get("ref_alias")
                if ref_alias:
                    referenced.add(ref_alias)

        def _mark_expr(expr: str) -> None:
            if not expr:
                return
            tokens = re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\\.", expr)
            for tok in tokens:
                referenced.add(tok)
            if expr in ir.nodes:
                referenced.add(expr)

        for item in ir.with_items + ir.with_filters:
            _mark_expr(item)
        for ret in ir.returns:
            _mark_expr(ret.expr)
        for order in ir.order_by:
            _mark_expr(order.expr)

        if referenced:
            ir.nodes = {alias: node for alias, node in ir.nodes.items() if alias in referenced}

    def _extract_label_hints(self, hints: List[str]) -> List[Tuple[str, str, str]]:
        """
        Convert structural hints like 'Airport-[:ORIGIN]->Airport' or chained
        strings 'Customer-[:PLACED]->Order -> Order-[:HAS_ITEM]->OrderItem'
        into label-level tuples (src, rel, dst).
        """
        triples: List[Tuple[str, str, str]] = []
        pattern = re.compile(r"([A-Za-z0-9_`]+)-\[:([A-Za-z0-9_`]+)\]->([A-Za-z0-9_`]+)")
        for hint in hints:
            for match in pattern.finditer(hint):
                triples.append((match.group(1), match.group(2), match.group(3)))
        seen: set[Tuple[str, str, str]] = set()
        ordered: List[Tuple[str, str, str]] = []
        for t in triples:
            if t not in seen:
                seen.add(t)
                ordered.append(t)
        return ordered

    def _normalize_aliases(self, ir: ISOQueryIR) -> Dict[str, str]:
        reserved = {"match", "where", "return", "order", "limit", "with", "and", "or", "not"}
        mapping: Dict[str, str] = {}

        def _sanitize(alias: str) -> str:
            sanitized = re.sub(r"[^a-z0-9_]", "_", alias.lower())
            if not sanitized or sanitized[0].isdigit():
                sanitized = f"n_{sanitized}" if sanitized else "n"
            while (
                sanitized.lower() in reserved
                or sanitized in mapping.values()
                or (sanitized in ir.nodes and sanitized != alias)
            ):
                sanitized += "1"
            return sanitized

        for alias in list(ir.nodes.keys()):
            safe = _sanitize(alias)
            if safe != alias:
                mapping[alias] = safe

        if not mapping:
            return {}

        new_nodes: Dict[str, IRNode] = {}
        for alias, node in ir.nodes.items():
            new_alias = mapping.get(alias, alias)
            new_nodes[new_alias] = IRNode(alias=new_alias, label=node.label)
        ir.nodes = new_nodes

        for edge in ir.edges:
            edge.left_alias = mapping.get(edge.left_alias, edge.left_alias)
            edge.right_alias = mapping.get(edge.right_alias, edge.right_alias)

        for flt in ir.filters:
            flt.alias = mapping.get(flt.alias, flt.alias)
            if isinstance(flt.value, dict) and "ref_alias" in flt.value:
                flt.value["ref_alias"] = mapping.get(flt.value["ref_alias"], flt.value["ref_alias"])

        def _replace_alias_tokens(expr: str) -> str:
            updated = expr
            for old, new in mapping.items():
                try:
                    updated = re.sub(rf"\b{re.escape(old)}\.", f"{new}.", updated)
                    updated = re.sub(rf"\b{re.escape(old)}\b", new, updated)
                except re.error:
                    continue
            return updated

        ir.with_items = [_replace_alias_tokens(item) for item in ir.with_items]
        ir.with_filters = [_replace_alias_tokens(item) for item in ir.with_filters]
        ir.returns = [IRReturn(expr=_replace_alias_tokens(ret.expr), alias=ret.alias) for ret in ir.returns]
        ir.order_by = [IROrder(expr=_replace_alias_tokens(o.expr), direction=o.direction) for o in ir.order_by]

        return mapping

    def _role_conflicts(self, ir: ISOQueryIR) -> List[str]:
        """
        Detect ambiguous alias reuse where different relationship roles share the
        same target alias from the same source alias. This is schema-agnostic but
        catches cases like ORIGIN and DESTINATION pointing to the same airport alias.
        """
        conflicts: List[str] = []
        edge_map: Dict[Tuple[str, str], Set[str]] = defaultdict(set)
        for edge in ir.edges:
            key = (edge.left_alias, edge.right_alias)
            edge_map[key].add(edge.rel)
        for (left, right), rels in edge_map.items():
            if len(rels) > 1:
                conflicts.append(f"ambiguous role reuse between {left} and {right}: {', '.join(sorted(rels))}")
        return conflicts

    def _llm_fix_query(
        self,
        nl: str,
        schema_summary: str,
        contract: RequirementContract,
        query: str,
        errors: List[str],
        hints: List[str],
    ) -> Optional[str]:
        error_text = "- " + "\n- ".join(errors) if errors else "none"
        hint_text = "- " + "\n- ".join(hints) if hints else "none"
        contract_text = json.dumps(contract_view(contract), indent=2)
        user = (
            "You are fixing an ISO GQL query so it exactly satisfies the request and schema.\n"
            "Preserve only valid labels/relationships/properties from the schema summary.\n"
            "Keep aggregates grouped; keep required metrics/order/limits from the contract.\n"
            "Return ONLY the corrected ISO GQL query, no explanation.\n\n"
            f"NATURAL LANGUAGE:\n{nl}\n\nSCHEMA SUMMARY:\n{schema_summary}\n\n"
            f"CONTRACT:\n{contract_text}\n\nCURRENT QUERY:\n{query}\n\n"
            f"ISSUES:\n{error_text}\n\nSTRUCTURAL HINTS:\n{hint_text}"
        )
        try:
            fixed, _ = chat_complete(
                DEFAULT_OPENAI_MODEL_FIX,
                "Repair the ISO GQL query. Output only the fixed query, nothing else.",
                user,
                temperature=0.0,
                top_p=0.25,
                max_tokens=900,
            )
            return clean_block(fixed)
        except Exception:
            return None

    def _evaluate_candidate(
        self,
        nl: str,
        pre: PreprocessResult,
        candidate: CandidateQuery,
        schema_validator: SchemaGroundingValidator,
        hints: List[str],
        link_guidance: Optional[Dict[str, Any]],
        contract: RequirementContract,
        label_hints: Optional[List[Tuple[str, str, str]]] = None,
    ) -> ValidationBundle:
        label_hints = label_hints or []

        def _evaluate_text(raw_query: str) -> ValidationBundle:
            # Normalize common malformed path snippets like `n1:Label-[:REL]->(n2:Label)`
            normalized_query = re.sub(
                r"([A-Za-z_][A-Za-z0-9_]*:[A-Za-z0-9_]+)\s*-\s*\[:\s*([A-Za-z0-9_]+)\s*\]\s*->",
                r"(\\1)-[:\\2]->",
                raw_query,
            )
            ir, parse_errors = ISOQueryIR.parse(normalized_query)
            repaired = False
            schema_errors: List[str] = []
            structural_errors: List[str] = []
            rendered = raw_query
            if ir:
                alias_label_hints: Dict[str, str] = {}
                rel_hints: List[Tuple[str, str, str]] = []
                if link_guidance:
                    for nl in link_guidance.get("node_links", []) or []:
                        alias, label = nl.get("alias"), nl.get("label")
                        if alias and label:
                            alias_label_hints[alias] = label
                    for rl in link_guidance.get("rel_links", []) or []:
                        left, rel, right = rl.get("left_alias"), rl.get("rel"), rl.get("right_alias")
                        if left and rel and right:
                            rel_hints.append((left, rel, right))
                alias_mapping = self._normalize_aliases(ir)
                if alias_mapping:
                    alias_label_hints = {alias_mapping.get(a, a): label for a, label in alias_label_hints.items()}
                    rel_hints = [
                        (alias_mapping.get(left, left), rel, alias_mapping.get(right, right))
                        for left, rel, right in rel_hints
                    ]
                    repaired = True
                repaired = (
                    self._repair_ir_schema(
                        ir, alias_label_hints=alias_label_hints, rel_hints=rel_hints, label_hints=label_hints
                    )
                    or repaired
                )
                # Enforce contract-required structure before validation/rendering.
                self._enforce_contract_structure(ir, contract)
                self._dedupe_edges(ir)
                self._prune_returns(ir)
                self._prune_unconnected_nodes(ir)
                # Normalize grouping if aggregates + non-aggregates are mixed.
                self._ensure_grouping(ir)
                self._ensure_order_fields(ir)
                structural_errors = validate_structure(normalized_query, ir) + self._role_conflicts(ir)
                schema_errors = schema_validator.validate(ir)
                rendered = ir.render()
            coverage_errors: List[str] = []
            if ir:
                coverage_errors = coverage_violations(contract, ir, rendered)
            syntax = self.runner.validate(rendered)
            logic_valid = False
            logic_reason: Optional[str] = None
            if ir:
                logic_valid, logic_reason = self.logic_validator.validate(
                    nl, pre.filtered_schema.summary_lines(), rendered, hints
                )
            return ValidationBundle(
                ir=ir,
                parse_errors=parse_errors,
                structural_errors=structural_errors,
                schema_errors=schema_errors,
                coverage_errors=coverage_errors,
                syntax_result=syntax,
                logic_valid=logic_valid,
                logic_reason=logic_reason,
                repaired=repaired,
                query_text=rendered,
            )

        def _score_bundle(bundle: ValidationBundle) -> int:
            return (
                len(bundle.parse_errors)
                + len(bundle.structural_errors)
                + len(bundle.schema_errors)
                + len(bundle.coverage_errors)
                + (0 if bundle.syntax_result.ok else 1)
                + (0 if bundle.logic_valid else 1)
            )

        def _edge_consistency_hints(ir: Optional[ISOQueryIR]) -> List[str]:
            if not ir:
                return []
            hints_local: List[str] = []
            label_hint_set = set(label_hints)
            rel_options_by_pair: Dict[Tuple[str, str], set[str]] = defaultdict(set)
            for src, rel, dst in label_hint_set:
                rel_options_by_pair[(src, dst)].add(rel)
            edges_by_pair: Dict[Tuple[Optional[str], Optional[str]], List[str]] = defaultdict(list)
            for e in ir.edges:
                l_lbl = ir.nodes.get(e.left_alias, IRNode(alias=e.left_alias)).label
                r_lbl = ir.nodes.get(e.right_alias, IRNode(alias=e.right_alias)).label
                edges_by_pair[(l_lbl, r_lbl)].append(e.rel)
            for pair, rels in edges_by_pair.items():
                options = rel_options_by_pair.get(pair)
                if not options or len(options) < 2:
                    continue
                distinct_used = set(rels)
                if len(distinct_used) < len(options):
                    missing = ", ".join(sorted(options - distinct_used))
                    hints_local.append(f"missing relationship(s) for {pair[0]}->{pair[1]}: {missing}")
                if any(rels.count(rel) > 1 for rel in distinct_used) and len(rels) > len(options):
                    hints_local.append(
                        f"duplicated relationship(s) for {pair[0]}->{pair[1]}: " + ", ".join(sorted(distinct_used))
                    )
            return hints_local

        initial_bundle = _evaluate_text(candidate.query)
        fix_records: List[Dict[str, Any]] = []
        current_bundle = initial_bundle

        # STEP 1: Deterministic schema repair FIRST (free, no LLM cost)
        # This fixes edge directions before we waste LLM tokens
        if current_bundle.ir and current_bundle.schema_errors:
            schema_repair_changed = self._deterministic_schema_repair(current_bundle.ir)
            if schema_repair_changed:
                repaired_query = current_bundle.ir.render()
                current_bundle = _evaluate_text(repaired_query)
                current_bundle.fix_applied = True
                current_bundle.fix_details = "deterministic_schema_repair"
                fix_records.append({
                    "note": "deterministic_schema_repair",
                    "input": initial_bundle.query_text,
                    "output": repaired_query,
                    "issues": initial_bundle.schema_errors,
                })

        # STEP 2: LLM fix for remaining non-schema issues (logic, coverage, etc.)
        remaining_errors = current_bundle.parse_errors + current_bundle.schema_errors + current_bundle.coverage_errors
        edge_hints = _edge_consistency_hints(current_bundle.ir)
        needs_llm_fix = (remaining_errors or not current_bundle.logic_valid or edge_hints) and current_bundle.ir

        if needs_llm_fix:
            fix_issues = list(remaining_errors) + edge_hints
            if not current_bundle.logic_valid and current_bundle.logic_reason:
                fix_issues.append(f"logic: {current_bundle.logic_reason}")

            fixed_query = self._llm_fix_query(
                nl, pre.filtered_schema.summary_lines(), contract, current_bundle.query_text, fix_issues, hints
            )

            if fixed_query and fixed_query.strip() and fixed_query.strip() != current_bundle.query_text.strip():
                llm_fixed_bundle = _evaluate_text(fixed_query)
                llm_fixed_bundle.fix_applied = True
                llm_fixed_bundle.fix_details = "llm_fix"
                fix_records.append({
                    "note": "llm_fix",
                    "input": current_bundle.query_text,
                    "output": fixed_query,
                    "issues": fix_issues,
                })

                # STEP 3: Deterministic schema repair AGAIN as safety net after LLM fix
                # LLM might have introduced new schema errors
                if llm_fixed_bundle.ir and llm_fixed_bundle.schema_errors:
                    post_llm_repair_changed = self._deterministic_schema_repair(llm_fixed_bundle.ir)
                    if post_llm_repair_changed:
                        final_repaired_query = llm_fixed_bundle.ir.render()
                        llm_fixed_bundle = _evaluate_text(final_repaired_query)
                        llm_fixed_bundle.fix_applied = True
                        llm_fixed_bundle.fix_details = "deterministic_schema_repair"
                        fix_records.append({
                            "note": "deterministic_schema_repair",
                            "input": fixed_query,
                            "output": final_repaired_query,
                            "issues": ["post-llm schema cleanup"],
                        })

                # Keep the better bundle
                if _score_bundle(llm_fixed_bundle) <= _score_bundle(current_bundle):
                    current_bundle = llm_fixed_bundle

        # Attach fix records to final bundle
        current_bundle.fixes = fix_records if fix_records else []
        final_bundle = current_bundle

        # Return a bundle that includes pre/post for logging clarity
        class EnhancedBundle(ValidationBundle):
            def __init__(self, pre_fix_bundle, fixes, post_fix_bundle):
                super().__init__(
                    ir=post_fix_bundle.ir,
                    parse_errors=post_fix_bundle.parse_errors,
                    structural_errors=post_fix_bundle.structural_errors,
                    schema_errors=post_fix_bundle.schema_errors,
                    coverage_errors=post_fix_bundle.coverage_errors,
                    syntax_result=post_fix_bundle.syntax_result,
                    logic_valid=post_fix_bundle.logic_valid,
                    logic_reason=post_fix_bundle.logic_reason,
                    repaired=post_fix_bundle.repaired,
                    query_text=post_fix_bundle.query_text,
                    fix_applied=post_fix_bundle.fix_applied,
                    fix_details=post_fix_bundle.fix_details,
                    fixes=fixes,
                )
                self.pre_fix_bundle = pre_fix_bundle
                self.post_fix_bundle = post_fix_bundle

        return EnhancedBundle(initial_bundle, fix_records, final_bundle)

    def run(
        self,
        nl: str,
        preprocessor: Preprocessor,
        intent_linker,
        spinner: Optional[Spinner],
        *,
        trace_path: Optional[str] = None,
        run_logger: Optional[RunLogger] = None,
    ) -> Tuple[str, List[Dict[str, Any]]]:
        failures: List[str] = []
        timeline: List[Dict[str, any]] = []
        schema_validator = SchemaGroundingValidator(self.graph)
        trace_dir = Path(trace_path) if trace_path else (run_logger.trace_dir if run_logger else None)
        if trace_dir:
            trace_dir.mkdir(parents=True, exist_ok=True)
        feedback_used = False
        best_query: Optional[str] = None
        best_score: Optional[int] = None
        cached_guidance: Optional[IntentLinkGuidance] = None

        with self.runner:
            for attempt in range(1, self.max_loops + 1):
                if spinner:
                    spinner.set_attempt(attempt)
                    spinner.set_stage("understand", "preprocessing input...")
                pre = preprocessor.run(nl, failures)
                if spinner:
                    detail = "analyzing intent & linking schema..." if cached_guidance is None else "reusing cached intent/links..."
                    spinner.set_stage("understand", detail)
                guidance = cached_guidance or intent_linker.run(nl, pre, failures)
                if cached_guidance is None:
                    cached_guidance = guidance
                contract = build_contract(nl, pre, guidance, self.graph)
                timeline.append({"attempt": attempt, "phase": "intent", "frame": guidance.frame})
                timeline.append({"attempt": attempt, "phase": "link", "links": guidance.links})
                timeline.append({"attempt": attempt, "phase": "contract", "requirements": contract_view(contract)})

                frame_hints = guidance.frame.get("path_hints") if isinstance(guidance.frame, dict) else None
                link_hints = links_to_hints(guidance.links)
                contract_hints = [f"{src}-[:{rel}]->{dst}" for (src, rel, dst) in contract.required_edges]
                frame_hints = guidance.frame.get("path_hints") if isinstance(guidance.frame, dict) else None
                combined_hints = link_hints + contract_hints + (frame_hints or [])
                logic_hints = sorted(set(h for h in combined_hints if h))[:8]
                label_hints = self._extract_label_hints(pre.filtered_schema.path_hints + contract_hints)
                timeline.append(
                    {
                        "attempt": attempt,
                        "phase": "hints",
                        "logic_hints": logic_hints,
                        "link_hints": link_hints,
                        "frame_hints": frame_hints or [],
                    }
                )

                if spinner:
                    spinner.set_stage("contract", "requirements built")
                    spinner.set_stage("generate", "LLM generating candidates...")

                # Some test stubs still expose a 3-arg generator; prefer passing the
                # contract when supported, but fall back gracefully.
                gen_params = list(inspect.signature(self.generator.generate).parameters.values())
                accepts_contract = any(
                    p.kind in (p.VAR_KEYWORD, p.VAR_POSITIONAL)
                    or p.name == "contract"
                    or p.name == "trace"
                    for p in gen_params[1:]  # skip self for bound methods
                )
                gen_trace: Dict[str, Any] = {}
                if accepts_contract:
                    candidates = self.generator.generate(pre, failures, guidance, contract=contract, trace=gen_trace)
                else:  # pragma: no cover - compatibility with older stubs
                    candidates = self.generator.generate(pre, failures, guidance)
                timeline.append({"attempt": attempt, "phase": "generate", "candidates": [c.query for c in candidates]})
                if not candidates:
                    if trace_dir and gen_trace:
                        attempt_trace = {
                            "attempt": attempt,
                            "nl": nl,
                            "pre": {
                                "normalized": pre.normalized_nl,
                                "hints": pre.structural_hints,
                                "schema_summary": pre.filtered_schema.summary_lines(),
                            },
                            "intent_frame": guidance.frame,
                            "links": guidance.links,
                            "contract": contract_view(contract),
                            "logic_hints": logic_hints,
                            "label_hints": label_hints,
                            "generator_prompt": gen_trace.get("prompt"),
                            "generator_raw": gen_trace.get("raw"),
                            "candidates": [],
                        }
                        if run_logger:
                            run_logger.log_attempt_trace(attempt, attempt_trace, empty=True)
                        else:
                            (trace_dir / f"attempt_{attempt}_empty.json").write_text(
                                json.dumps(attempt_trace, indent=2), encoding="utf-8"
                            )
                    failures.append("generator returned no candidates")
                    continue

                attempt_trace: Dict[str, Any] = {}
                if trace_dir:
                    attempt_trace = {
                        "attempt": attempt,
                        "nl": nl,
                        "pre": {
                            "normalized": pre.normalized_nl,
                            "hints": pre.structural_hints,
                            "schema_summary": pre.filtered_schema.summary_lines(),
                        },
                        "intent_frame": guidance.frame,
                        "links": guidance.links,
                        "contract": contract_view(contract),
                        "logic_hints": logic_hints,
                        "label_hints": label_hints,
                        "generator_prompt": gen_trace.get("prompt"),
                        "generator_raw": gen_trace.get("raw"),
                    }

                if spinner:
                    spinner.set_stage("validate", f"checking {len(candidates)} candidate(s)...")

                for idx_candidate, candidate in enumerate(candidates, start=1):
                    if spinner:
                        spinner.set_stage("validate", f"validating candidate {idx_candidate}/{len(candidates)}...")
                    bundle = self._evaluate_candidate(
                        nl, pre, candidate, schema_validator, logic_hints, guidance.links, contract, label_hints
                    )
                    pre_bundle = getattr(bundle, 'pre_fix_bundle', bundle)
                    post_bundle = getattr(bundle, 'post_fix_bundle', bundle)
                    fixes = getattr(bundle, 'fixes', [])
                    
                    # Update spinner with validation results
                    if spinner:
                        pre_errors = pre_bundle.schema_errors + pre_bundle.coverage_errors + pre_bundle.parse_errors
                        for err in pre_errors[:2]:  # Show first 2 errors
                            spinner.add_error(err[:80] if len(err) > 80 else err)
                        
                        if fixes:
                            spinner.set_stage("repair", "applying fixes...")
                            for fix in fixes:
                                fix_note = fix.get("note", "unknown")
                                spinner.add_fix(fix_note)

                    self._persist_debug(
                        run_logger,
                        f"attempt{attempt}",
                        {
                            "nl": nl,
                            "attempt": attempt,
                            "contract": contract_view(contract),
                            "candidate": candidate.query,
                            "pre_fix_coverage_errors": pre_bundle.coverage_errors,
                            "pre_fix_schema_errors": pre_bundle.schema_errors,
                            "pre_fix_parse_errors": pre_bundle.parse_errors,
                            "pre_fix_logic_valid": pre_bundle.logic_valid,
                            "pre_fix_logic_reason": pre_bundle.logic_reason,
                            "post_fix_coverage_errors": post_bundle.coverage_errors,
                            "post_fix_schema_errors": post_bundle.schema_errors,
                            "post_fix_parse_errors": post_bundle.parse_errors,
                            "post_fix_logic_valid": post_bundle.logic_valid,
                            "post_fix_logic_reason": post_bundle.logic_reason,
                            "logic_hints": logic_hints,
                            "rendered": post_bundle.query_text,
                            "fix_applied": post_bundle.fix_applied,
                            "fix_details": post_bundle.fix_details,
                        },
                    )
                    timeline.append(
                        {
                            "attempt": attempt,
                            "raw_query": candidate.query,
                            "pre_fix_bundle": {
                                "query": pre_bundle.query_text,
                                "parse_errors": pre_bundle.parse_errors,
                                "structural_errors": pre_bundle.structural_errors,
                                "schema_errors": pre_bundle.schema_errors,
                                "coverage_errors": pre_bundle.coverage_errors,
                                "syntax_ok": pre_bundle.syntax_result.ok,
                                "syntax_error": pre_bundle.syntax_result.error,
                                "logic_valid": pre_bundle.logic_valid,
                                "logic_reason": pre_bundle.logic_reason,
                                "repaired": pre_bundle.repaired,
                            },
                            "fixes": fixes,
                            "post_fix_bundle": {
                                "query": post_bundle.query_text,
                                "parse_errors": post_bundle.parse_errors,
                                "structural_errors": post_bundle.structural_errors,
                                "schema_errors": post_bundle.schema_errors,
                                "coverage_errors": post_bundle.coverage_errors,
                                "syntax_ok": post_bundle.syntax_result.ok,
                                "syntax_error": post_bundle.syntax_result.error,
                                "logic_valid": post_bundle.logic_valid,
                                "logic_reason": post_bundle.logic_reason,
                                "repaired": post_bundle.repaired,
                                "fix_applied": post_bundle.fix_applied,
                                "fix_details": post_bundle.fix_details,
                            },
                        }
                    )

                    if trace_dir:
                        pre_bundle = getattr(bundle, 'pre_fix_bundle', bundle)
                        post_bundle = getattr(bundle, 'post_fix_bundle', bundle)
                        fixes = getattr(bundle, 'fixes', [])
                        attempt_trace.setdefault("candidates", []).append(
                            {
                                "raw": candidate.query,
                                "pre_fix_rendered": pre_bundle.query_text,
                                "pre_fix_parse_errors": pre_bundle.parse_errors,
                                "pre_fix_structural_errors": pre_bundle.structural_errors,
                                "pre_fix_schema_errors": pre_bundle.schema_errors,
                                "pre_fix_coverage_errors": pre_bundle.coverage_errors,
                                "pre_fix_syntax_ok": pre_bundle.syntax_result.ok,
                                "pre_fix_syntax_error": pre_bundle.syntax_result.error,
                                "pre_fix_logic_valid": pre_bundle.logic_valid,
                                "pre_fix_logic_reason": pre_bundle.logic_reason,
                                "fixes": fixes,
                                "post_fix_rendered": post_bundle.query_text,
                                "post_fix_parse_errors": post_bundle.parse_errors,
                                "post_fix_structural_errors": post_bundle.structural_errors,
                                "post_fix_schema_errors": post_bundle.schema_errors,
                                "post_fix_coverage_errors": post_bundle.coverage_errors,
                                "post_fix_syntax_ok": post_bundle.syntax_result.ok,
                                "post_fix_syntax_error": post_bundle.syntax_result.error,
                                "post_fix_logic_valid": post_bundle.logic_valid,
                                "post_fix_logic_reason": post_bundle.logic_reason,
                                "fix_applied": post_bundle.fix_applied,
                                "fix_details": post_bundle.fix_details,
                            }
                        )

                    logic_ok = bundle.logic_valid
                    syntax_ok = bundle.syntax_result.ok or (
                        bundle.syntax_result.error
                        and not bundle.parse_errors
                        and not bundle.schema_errors
                        and not bundle.coverage_errors
                    )
                    all_clear = (
                        bundle.ir is not None
                        and not bundle.parse_errors
                        and not bundle.structural_errors
                        and not bundle.schema_errors
                        and not bundle.coverage_errors
                        and syntax_ok
                        and logic_ok
                    )
                    if all_clear:
                        if spinner:
                            spinner.set_stage("finalize", "query validated successfully!")
                        timeline.append(
                            {
                                "attempt": attempt,
                                "phase": "final",
                                "status": "success",
                                "query": bundle.ir.render(),
                            }
                        )
                        # Log trace for successful attempt
                        if trace_dir and attempt_trace:
                            if run_logger:
                                run_logger.log_attempt_trace(attempt, attempt_trace)
                            else:
                                (trace_dir / f"attempt_{attempt}.json").write_text(
                                    json.dumps(attempt_trace, indent=2), encoding="utf-8"
                                )
                        return bundle.ir.render(), timeline

                    # Log trace for failed attempt but continue to next attempt
                    if trace_dir and attempt_trace:
                        if run_logger:
                            run_logger.log_attempt_trace(attempt, attempt_trace)
                        else:
                            (trace_dir / f"attempt_{attempt}.json").write_text(
                                json.dumps(attempt_trace, indent=2), encoding="utf-8"
                            )

                    # Track the least-bad candidate to allow a graceful fallback.
                    score = (
                        len(bundle.parse_errors)
                        + len(bundle.structural_errors)
                        + len(bundle.schema_errors)
                        + len(bundle.coverage_errors)
                        + (0 if bundle.syntax_result.ok else 1)
                        + (0 if bundle.logic_valid else 1)
                    )
                    if best_score is None or score < best_score:
                        if (
                            not bundle.parse_errors
                            and not bundle.structural_errors
                            and not bundle.schema_errors
                            and not bundle.coverage_errors
                            and bundle.syntax_result.ok
                        ):
                            best_score = score
                            best_query = bundle.query_text or candidate.query

                    combined_reasons = (
                        bundle.parse_errors + bundle.structural_errors + bundle.schema_errors + bundle.coverage_errors
                    )
                    if not bundle.syntax_result.ok and bundle.syntax_result.error:
                        combined_reasons.append(f"syntax: {bundle.syntax_result.error}")
                    if not bundle.logic_valid and bundle.logic_reason:
                        combined_reasons.append(f"logic: {bundle.logic_reason}")
                    if not combined_reasons:
                        combined_reasons.append("unspecified failure")
                    failures.append("; ".join(sorted(set(combined_reasons))))

                if trace_dir and attempt_trace:
                    if run_logger:
                        run_logger.log_attempt_trace(attempt, attempt_trace)
                    else:
                        (trace_dir / f"attempt_{attempt}.json").write_text(
                            json.dumps(attempt_trace, indent=2), encoding="utf-8"
                        )

                # Single explicit feedback round after first unsuccessful attempt.
                if attempt == 1 and not feedback_used and failures:
                    feedback_used = True
                    feedback_note = self._llm_feedback(nl, pre.filtered_schema.summary_lines(), failures)
                    if feedback_note:
                        failures.append(f"LLM feedback: {feedback_note}")

        if best_query:
            # Return the best-effort query even if validation never fully cleared.
            return best_query, timeline

        raise PipelineFailure("pipeline failed after refinement loops", timeline, failures)

    def _llm_feedback(self, nl: str, schema_summary: str, failures: List[str]) -> Optional[str]:
        """Ask the LLM to summarize missing joins/metrics/order/limits as concise hints."""
        try:
            prompt = (
                "You are reviewing a failed ISO GQL attempt.\n"
                "Given the natural language request, schema summary, and the previous errors, produce 3-6 short hints "
                "that identify missing joins, metrics, order/limit, or grouping problems. "
                "Keep each hint to a single line; no new query text.\n\n"
                f"NATURAL LANGUAGE:\n{nl}\n\nSCHEMA SUMMARY:\n{schema_summary}\n\n"
                f"FAILURES:\n- " + "\n- ".join(failures[-6:])
            )
            feedback, _ = chat_complete(
                self.logic_validator.model,
                "Return concise bullet hints only.",
                prompt,
                temperature=0.0,
                top_p=0.2,
                max_tokens=200,
            )
            return feedback.strip()
        except Exception:
            return None


__all__ = ["Refiner", "ValidationBundle", "PipelineFailure"]


