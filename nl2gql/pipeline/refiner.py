from __future__ import annotations

import re
from collections import defaultdict
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple

from .generator import CandidateQuery, QueryGenerator
from .intent_linker import IntentLinkGuidance, links_to_hints
from .ir import IRFilter, IREdge, IRNode, ISOQueryIR, IROrder, IRReturn
from .preprocess import PreprocessResult, Preprocessor
from .runner import GraphLiteRunner, SyntaxResult
from .schema_graph import SchemaGraph
from .ui import Spinner
from .validators import LogicValidator, SchemaGroundingValidator


@dataclass
class ValidationBundle:
    ir: Optional[ISOQueryIR]
    parse_errors: List[str]
    schema_errors: List[str]
    syntax_result: SyntaxResult
    logic_valid: bool
    logic_reason: Optional[str]
    repaired: bool = False
    query_text: str = ""


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
        max_loops: int = 3,
    ) -> None:
        self.graph = graph
        self.generator = generator
        self.logic_validator = logic_validator or LogicValidator()
        self.runner = runner or GraphLiteRunner(db_path=db_path)
        self.max_loops = max_loops

    def _repair_ir_schema(
        self,
        ir: ISOQueryIR,
        *,
        alias_label_hints: Optional[Dict[str, str]] = None,
        rel_hints: Optional[List[Tuple[str, str, str]]] = None,
    ) -> bool:
        changed = False
        alias_label_hints = alias_label_hints or {}
        rel_hints = rel_hints or []

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

        def _normalize_temporal_literal(val: str) -> str:
            lower = val.lower()
            if not any(token in lower for token in {"date", "now", "last", "current"}):
                return val
            day_match = re.search(r"(\d+)\s*(?:day|days)", lower)
            if day_match:
                days = day_match.group(1)
                return f"date() - duration('P{days}D')"
            week_match = re.search(r"(\d+)\s*(?:week|weeks)", lower)
            if week_match:
                weeks = week_match.group(1)
                try:
                    days = int(weeks) * 7
                    return f"date() - duration('P{days}D')"
                except ValueError:
                    return val
            return val

        for flt in ir.filters:
            if isinstance(flt.value, str):
                normalized = _normalize_temporal_literal(flt.value)
                if normalized != flt.value:
                    flt.value = normalized
                    changed = True

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
            if left_label and right_label and any(
                e.src == right_label and e.rel == edge.rel and e.dst == left_label for e in self.graph.edges
            ):
                edge.left_alias, edge.right_alias = edge.right_alias, edge.left_alias
                changed = True
        return changed

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

    def _heuristic_logic_accept(self, ir: ISOQueryIR, hints: List[str]) -> bool:
        if not hints:
            return False

        edge_hints = {h for h in hints if "-[:".lower() in h.lower()}
        label_hints = {h for h in hints if ":" in h and "-[:".lower() not in h.lower()}

        ir_edge_tokens = {f"{e.left_alias.lower()}-[:{e.rel.lower()}]->{e.right_alias.lower()}" for e in ir.edges}
        ir_label_edge_tokens = set()
        for edge in ir.edges:
            left_label = ir.nodes.get(edge.left_alias).label if ir.nodes.get(edge.left_alias) else None
            right_label = ir.nodes.get(edge.right_alias).label if ir.nodes.get(edge.right_alias) else None
            if left_label and right_label:
                ir_label_edge_tokens.add(f"{left_label.lower()}-[:{edge.rel.lower()}]->{right_label.lower()}")
        ir_edge_tokens |= ir_label_edge_tokens

        ir_label_tokens = {f"{alias.lower()}:{node.label.lower()}" for alias, node in ir.nodes.items() if node.label}

        def _match_ratio(hint_set: set, token_set: set) -> float:
            if not hint_set:
                return 1.0
            matches = sum(1 for h in hint_set if h.lower() in token_set)
            return matches / len(hint_set)

        edge_ratio = _match_ratio(edge_hints, ir_edge_tokens)
        label_ratio = _match_ratio(label_hints, ir_label_tokens)
        has_returns = bool(ir.returns)

        return has_returns and edge_ratio >= 0.6 and label_ratio >= 0.6

    def _evaluate_candidate(
        self,
        nl: str,
        pre: PreprocessResult,
        candidate: CandidateQuery,
        schema_validator: SchemaGroundingValidator,
        hints: List[str],
        link_guidance: Optional[Dict[str, Any]],
    ) -> ValidationBundle:
        ir, parse_errors = ISOQueryIR.parse(candidate.query)
        repaired = False
        schema_errors: List[str] = []
        rendered = candidate.query
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
                    (alias_mapping.get(left, left), rel, alias_mapping.get(right, right)) for left, rel, right in rel_hints
                ]
                repaired = True
            repaired = self._repair_ir_schema(ir, alias_label_hints=alias_label_hints, rel_hints=rel_hints) or repaired
            schema_errors = schema_validator.validate(ir)
            rendered = ir.render()
        syntax = self.runner.validate(rendered)
        logic_valid = False
        logic_reason: Optional[str] = None
        if ir:
            # Rely solely on the structured logic validator; avoid heuristic overrides
            # so obviously incomplete queries (missing metrics, wrong filters) do not slip through.
            logic_valid, logic_reason = self.logic_validator.validate(
                nl, pre.filtered_schema.summary_lines(), rendered, hints
            )
        return ValidationBundle(
            ir=ir,
            parse_errors=parse_errors,
            schema_errors=schema_errors,
            syntax_result=syntax,
            logic_valid=logic_valid,
            logic_reason=logic_reason,
            repaired=repaired,
            query_text=rendered,
        )

    def run(
        self,
        nl: str,
        preprocessor: Preprocessor,
        intent_linker,
        spinner: Optional[Spinner],
    ) -> Tuple[str, List[Dict[str, Any]]]:
        failures: List[str] = []
        timeline: List[Dict[str, any]] = []
        schema_validator = SchemaGroundingValidator(self.graph)

        with self.runner:
            for attempt in range(1, self.max_loops + 1):
                if spinner:
                    spinner.update(f"[attempt {attempt}] preprocessing...")
                pre = preprocessor.run(nl, failures)
                if spinner:
                    spinner.update(f"[attempt {attempt}] planning intent and links...")
                guidance = intent_linker.run(nl, pre, failures)
                timeline.append({"attempt": attempt, "phase": "intent", "frame": guidance.frame})
                timeline.append({"attempt": attempt, "phase": "link", "links": guidance.links})
                frame_hints = guidance.frame.get("path_hints") if isinstance(guidance.frame, dict) else None
                combined_hints = sorted(set(pre.structural_hints + links_to_hints(guidance.links) + (frame_hints or [])))
                if spinner:
                    spinner.update(f"[attempt {attempt}] generating candidates...")
                candidates = self.generator.generate(pre, failures, guidance)
                timeline.append({"attempt": attempt, "phase": "generate", "candidates": [c.query for c in candidates]})
                if not candidates:
                    failures.append("generator returned no candidates")
                    continue

                for candidate in candidates:
                    bundle = self._evaluate_candidate(
                        nl, pre, candidate, schema_validator, combined_hints, guidance.links
                    )
                    timeline.append(
                        {
                            "attempt": attempt,
                            "raw_query": candidate.query,
                            "query": bundle.query_text,
                            "parse_errors": bundle.parse_errors,
                            "schema_errors": bundle.schema_errors,
                            "syntax_ok": bundle.syntax_result.ok,
                            "syntax_error": bundle.syntax_result.error,
                            "logic_valid": bundle.logic_valid,
                            "logic_reason": bundle.logic_reason,
                            "repaired": bundle.repaired,
                        }
                    )

                    all_clear = (
                        bundle.ir is not None
                        and not bundle.parse_errors
                        and not bundle.schema_errors
                        and bundle.syntax_result.ok
                        and bundle.logic_valid
                    )
                    if all_clear:
                        if spinner:
                            spinner.update(f"[attempt {attempt}] success.")
                        return bundle.ir.render(), timeline

                    combined_reasons = bundle.parse_errors + bundle.schema_errors
                    if not bundle.syntax_result.ok and bundle.syntax_result.error:
                        combined_reasons.append(f"syntax: {bundle.syntax_result.error}")
                    if not bundle.logic_valid and bundle.logic_reason:
                        combined_reasons.append(f"logic: {bundle.logic_reason}")
                    if not combined_reasons:
                        combined_reasons.append("unspecified failure")
                    failures.append("; ".join(sorted(set(combined_reasons))))

        raise PipelineFailure("pipeline failed after refinement loops", timeline, failures)


__all__ = ["Refiner", "ValidationBundle", "PipelineFailure"]


