from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Set, Tuple


@dataclass
class IRNode:
    alias: str
    label: Optional[str] = None


@dataclass
class IREdge:
    left_alias: str
    rel: str
    right_alias: str


@dataclass
class IRFilter:
    alias: str
    prop: str
    op: str
    value: Any


@dataclass
class IRReturn:
    expr: str
    alias: Optional[str] = None


@dataclass
class IROrder:
    expr: str
    direction: str = "ASC"


@dataclass
class ISOQueryIR:
    nodes: Dict[str, IRNode] = field(default_factory=dict)
    edges: List[IREdge] = field(default_factory=list)
    filters: List[IRFilter] = field(default_factory=list)
    with_items: List[str] = field(default_factory=list)
    with_filters: List[str] = field(default_factory=list)
    having_filters: List[str] = field(default_factory=list)
    group_by: List[str] = field(default_factory=list)
    returns: List[IRReturn] = field(default_factory=list)
    order_by: List[IROrder] = field(default_factory=list)
    limit: Optional[int] = None

    @classmethod
    def parse(cls, query: str) -> Tuple[Optional["ISOQueryIR"], List[str]]:
        errors: List[str] = []
        text = query.strip()
        if not text:
            return None, ["empty query"]

        def _clean(token: Optional[str]) -> Optional[str]:
            if token is None:
                return None
            return token.strip("`").strip()

        def _parse_value(val_raw: str) -> Any:
            val_raw = val_raw.strip()
            if val_raw.lower() in {"true", "false"}:
                return val_raw.lower() == "true"
            if re.match(r"^-?\d+(\.\d+)?$", val_raw):
                return float(val_raw) if "." in val_raw else int(val_raw)
            if val_raw.startswith("'") and val_raw.endswith("'"):
                return val_raw.strip("'").replace("\\'", "'")
            return val_raw

        token_pattern = re.compile(
            r"\bMATCH\b|\bWHERE\b|\bWITH\b|\bRETURN\b|\bHAVING\b|\bGROUP\s+BY\b|\bORDER\s+BY\b|\bLIMIT\b", flags=re.IGNORECASE
        )
        tokens: List[Dict[str, Any]] = []
        for m in token_pattern.finditer(text):
            raw = m.group(0).upper()
            label = "ORDER BY" if "ORDER" in raw else raw
            tokens.append({"label": label, "start": m.start(), "end": m.end()})
        tokens.sort(key=lambda t: t["start"])

        match_tokens = [t for t in tokens if t["label"] == "MATCH"]

        def _block(label: str, *, after: int = -1) -> Tuple[str, Optional[Dict[str, Any]]]:
            for tok in tokens:
                if tok["label"] == label and tok["start"] > after:
                    next_starts = [t["start"] for t in tokens if t["start"] > tok["start"]]
                    end_idx = min(next_starts) if next_starts else len(text)
                    return text[tok["end"] : end_idx].strip(), tok
            return "", None

        def _blocks(label: str, *, after: int = -1) -> List[Tuple[str, Dict[str, Any]]]:
            blocks: List[Tuple[str, Dict[str, Any]]] = []
            for tok in tokens:
                if tok["label"] == label and tok["start"] > after:
                    next_starts = [t["start"] for t in tokens if t["start"] > tok["start"]]
                    end_idx = min(next_starts) if next_starts else len(text)
                    blocks.append((text[tok["end"] : end_idx].strip(), tok))
            return blocks

        match_blocks = _blocks("MATCH")
        match_end = match_tokens[-1]["end"] if match_tokens else 0
        with_block, with_tok = _block("WITH", after=match_end)

        def _block_before(label: str, limit: int) -> Tuple[str, Optional[Dict[str, Any]]]:
            for tok in tokens:
                if tok["label"] == label and tok["start"] > match_end and tok["start"] < limit:
                    next_starts = [t["start"] for t in tokens if t["start"] > tok["start"]]
                    end_idx = min(next_starts) if next_starts else len(text)
                    return text[tok["end"] : end_idx].strip(), tok
            return "", None

        where_block, where_tok = _block_before("WHERE", with_tok["start"] if with_tok else float("inf"))
        with_where_block, with_where_tok = _block("WHERE", after=with_tok["start"]) if with_tok else ("", None)
        # HAVING comes after WITH but before RETURN - search for it in the correct position
        having_block, having_tok = _block("HAVING", after=with_tok["start"]) if with_tok else _block("HAVING", after=match_end)
        return_block, return_tok = _block("RETURN", after=match_end)
        group_block, group_tok = _block("GROUP BY", after=with_tok["start"] if with_tok else match_end)
        order_block, order_tok = _block("ORDER BY", after=return_tok["start"] if return_tok else match_end)
        limit_block = ""
        limit_after = None
        if order_tok:
            limit_after = order_tok["start"]
        elif group_tok:
            limit_after = group_tok["start"]
        elif having_tok:
            limit_after = having_tok["start"]
        elif return_tok:
            limit_after = return_tok["start"]
        if limit_after is not None:
            limit_block, _ = _block("LIMIT", after=limit_after)

        nodes: Dict[str, IRNode] = {}
        edges: List[IREdge] = []
        filters: List[IRFilter] = []
        seen_edges: Set[Tuple[str, str, str]] = set()
        path_auto_idx = 0

        if match_blocks:
            node_pattern = re.compile(
                r"\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([A-Za-z0-9_`]+))?\s*(?:\{([^}]*)\})?\s*\)"
            )
            edge_forward = re.compile(
                r"(?=(\(\s*(?P<src>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<src_label>[A-Za-z0-9_`]+))?\s*(?:\{[^}]*\})?\s*\)"
                r"\s*-\s*\[:\s*(?P<rel>[A-Za-z0-9_`]+)\s*\]\s*->\s*"
                r"\(\s*(?P<dst>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<dst_label>[A-Za-z0-9_`]+))?\s*(?:\{[^}]*\})?\s*\)))"
            )
            edge_backward = re.compile(
                r"(?=(\(\s*(?P<left>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<left_label>[A-Za-z0-9_`]+))?\s*(?:\{[^}]*\})?\s*\)"
                r"\s*<-\s*\[:\s*(?P<rel>[A-Za-z0-9_`]+)\s*\]\s*-\s*"
                r"\(\s*(?P<right>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<right_label>[A-Za-z0-9_`]+))?\s*(?:\{[^}]*\})?\s*\)))"
            )

            for match_block, _ in match_blocks:
                for alias, label, props in node_pattern.findall(match_block):
                    existing = nodes.get(alias)
                    if not (existing and existing.label):
                        nodes[alias] = IRNode(alias=alias, label=_clean(label) or (existing.label if existing else None))
                    if props:
                        for assignment in props.split(","):
                            if ":" not in assignment:
                                continue
                            key, val = assignment.split(":", 1)
                            key = key.strip()
                            val = val.strip()
                            if key:
                                filters.append(IRFilter(alias=alias, prop=key, op="=", value=_parse_value(val)))

                for m in edge_forward.finditer(match_block):
                    src, src_label, rel, dst, dst_label = (
                        m.group("src"),
                        m.group("src_label"),
                        m.group("rel"),
                        m.group("dst"),
                        m.group("dst_label"),
                    )
                    src_label = _clean(src_label)
                    dst_label = _clean(dst_label)
                    rel = _clean(rel) or rel
                    nodes.setdefault(src, IRNode(alias=src, label=src_label))
                    nodes.setdefault(dst, IRNode(alias=dst, label=dst_label))
                    key = (src, rel, dst)
                    if key not in seen_edges:
                        seen_edges.add(key)
                        edges.append(IREdge(left_alias=src, rel=rel, right_alias=dst))

                for m in edge_backward.finditer(match_block):
                    left, left_label, rel, right, right_label = (
                        m.group("left"),
                        m.group("left_label"),
                        m.group("rel"),
                        m.group("right"),
                        m.group("right_label"),
                    )
                    src, dst = right, left
                    left_label = _clean(left_label)
                    right_label = _clean(right_label)
                    rel = _clean(rel) or rel
                    nodes.setdefault(src, IRNode(alias=src, label=right_label))
                    nodes.setdefault(dst, IRNode(alias=dst, label=left_label))
                    key = (src, rel, dst)
                    if key not in seen_edges:
                        seen_edges.add(key)
                        edges.append(IREdge(left_alias=src, rel=rel, right_alias=dst))
        else:
            errors.append("MATCH clause missing")

        if where_block:
            clauses = [c.strip() for c in re.split(r"\bAND\b", where_block, flags=re.IGNORECASE) if c.strip()]
            for clause in clauses:
                alias_compare = re.match(
                    r"^([A-Za-z_][A-Za-z0-9_]*)\s*(=|<>)\s*([A-Za-z_][A-Za-z0-9_]*)$", clause
                )
                cond_match = re.match(
                    r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)\s*(=|<>|<=|>=|<|>)\s*(.+)",
                    clause,
                )
                in_match = re.match(
                    r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)\s+IN\s+\[([^\]]+)\]",
                    clause,
                    flags=re.IGNORECASE,
                )
                null_match = re.match(
                    r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)\s+IS\s+(NOT\s+)?NULL", clause, flags=re.IGNORECASE
                )
                path_match = re.match(
                    r"\(\s*(?P<src>[A-Za-z_][A-Za-z0-9_]*)\s*\)\s*-\s*\[:\s*(?P<rel>[A-Za-z0-9_`]+)\s*\]\s*->\s*\(\s*(?:(?P<dst>[A-Za-z_][A-Za-z0-9_]*)\s*)?(?::\s*(?P<dst_label>[A-Za-z0-9_`]+))?\s*(?:\{(?P<props>[^}]*)\})?\s*\)",
                    clause,
                )
                if alias_compare:
                    left_alias, op, right_alias = alias_compare.groups()
                    filters.append(
                        IRFilter(
                            alias=left_alias,
                            prop="id",
                            op=op,
                            value={"ref_alias": right_alias, "ref_property": "id"},
                        )
                    )
                elif cond_match:
                    alias, prop, op, val_raw = cond_match.groups()
                    val_raw = val_raw.strip()
                    value: Any = val_raw
                    ref_match = re.match(r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)", val_raw)
                    if ref_match:
                        value = {"ref_alias": ref_match.group(1), "ref_property": ref_match.group(2)}
                    elif val_raw.lower() in {"true", "false"}:
                        value = val_raw.lower() == "true"
                    elif re.match(r"^-?\d+(\.\d+)?$", val_raw):
                        value = float(val_raw) if "." in val_raw else int(val_raw)
                    elif val_raw.startswith("'") and val_raw.endswith("'"):
                        value = val_raw.strip("'").replace("\\'", "'")
                    filters.append(IRFilter(alias=alias, prop=prop, op=op, value=value))
                elif in_match:
                    alias, prop, list_raw = in_match.groups()
                    values = []
                    for token in list_raw.split(","):
                        token = token.strip()
                        if not token:
                            continue
                        values.append(_parse_value(token))
                    filters.append(IRFilter(alias=alias, prop=prop, op="IN", value=values))
                elif null_match:
                    alias, prop, not_part = null_match.group(1), null_match.group(2), null_match.group(3)
                    op = "IS NOT NULL" if not_part else "IS NULL"
                    filters.append(IRFilter(alias=alias, prop=prop, op=op, value=None))
                elif path_match:
                    src_alias = path_match.group("src")
                    rel = _clean(path_match.group("rel")) or path_match.group("rel")
                    dst_alias = path_match.group("dst")
                    dst_label = _clean(path_match.group("dst_label")) or None
                    props_raw = path_match.group("props") or ""

                    nodes.setdefault(src_alias, IRNode(alias=src_alias))
                    if not dst_alias:
                        path_auto_idx += 1
                        dst_alias = f"path{path_auto_idx}"
                    dst_node = nodes.setdefault(dst_alias, IRNode(alias=dst_alias, label=dst_label))
                    if dst_label and not dst_node.label:
                        dst_node.label = dst_label

                    key = (src_alias, rel, dst_alias)
                    if key not in seen_edges:
                        seen_edges.add(key)
                        edges.append(IREdge(left_alias=src_alias, rel=rel, right_alias=dst_alias))

                    for assignment in props_raw.split(","):
                        if ":" not in assignment:
                            continue
                        key_raw, val_raw = assignment.split(":", 1)
                        key_raw = key_raw.strip()
                        val_raw = val_raw.strip()
                        if not key_raw:
                            continue
                        filters.append(IRFilter(alias=dst_alias, prop=key_raw, op="=", value=_parse_value(val_raw)))
                else:
                    errors.append(f"unparsed WHERE clause: {clause}")

        with_items: List[str] = []
        with_filters: List[str] = []
        having_filters: List[str] = []
        if with_block:
            with_items = [item.strip() for item in with_block.split(",") if item.strip()]
        if with_where_block:
            with_filters = [c.strip() for c in re.split(r"\bAND\b", with_where_block, flags=re.IGNORECASE) if c.strip()]
        if having_block:
            having_filters = [c.strip() for c in re.split(r"\bAND\b", having_block, flags=re.IGNORECASE) if c.strip()]

        if filters:
            unique_filters: List[IRFilter] = []
            seen_keys: Set[Tuple[str, str, str]] = set()
            for flt in filters:
                key = (flt.alias, flt.prop, f"{flt.op}:{flt.value}")
                if key not in seen_keys:
                    seen_keys.add(key)
                    unique_filters.append(flt)
            filters = unique_filters

        returns: List[IRReturn] = []
        if return_block:
            items = [i.strip() for i in return_block.split(",") if i.strip()]
            for item in items:
                if " AS " in item.upper():
                    parts = re.split(r"\s+AS\s+", item, flags=re.IGNORECASE)
                    expr, alias = parts[0].strip(), parts[1].strip()
                    returns.append(IRReturn(expr=expr, alias=alias))
                else:
                    returns.append(IRReturn(expr=item))
        else:
            errors.append("RETURN clause missing")

        order_by: List[IROrder] = []
        if order_block:
            items = [i.strip() for i in order_block.split(",") if i.strip()]
            for item in items:
                pieces = item.split()
                if pieces:
                    expr = pieces[0]
                    direction = pieces[1] if len(pieces) > 1 else "ASC"
                    order_by.append(IROrder(expr=expr, direction=direction.upper()))

        group_by: List[str] = []
        if group_block:
            group_by = [item.strip() for item in group_block.split(",") if item.strip()]

        limit: Optional[int] = int(limit_block) if limit_block.isdigit() else None

        if return_tok and with_tok and with_tok["start"] > return_tok["start"] and returns:
            with_items = [f"{r.expr} AS {r.alias}" if r.alias else r.expr for r in returns]
            returns = [IRReturn(expr=r.alias or r.expr) for r in returns]

        return (
            cls(
                nodes=nodes,
                edges=edges,
                filters=filters,
                with_items=with_items,
                with_filters=with_filters,
                having_filters=having_filters,
                group_by=group_by,
                returns=returns,
                order_by=order_by,
                limit=limit,
            ),
            errors,
        )

    def render(self) -> str:
        def _quote_rel(rel: str) -> str:
            # Only quote when the rel contains characters that require escaping.
            if re.match(r"^[A-Za-z0-9_]+$", rel or ""):
                return rel
            safe = (rel or "").replace("`", "``")
            return f"`{safe}`"

        def _format_value(val: Any) -> str:
            if isinstance(val, bool):
                return "true" if val else "false"
            if isinstance(val, (int, float)):
                return str(val)
            if isinstance(val, (list, tuple)):
                return "[" + ", ".join(_format_value(v) for v in val) + "]"
            if isinstance(val, dict) and "ref_alias" in val and "ref_property" in val:
                return f"{val['ref_alias']}.{val['ref_property']}"
            text = str(val).strip()
            if re.match(r"[A-Za-z_][A-Za-z0-9_]*\s*\(", text) or "duration(" in text or "date()" in text or "datetime()" in text:
                return text
            text = text.replace("\\", "\\\\").replace("'", "\\'")
            return f"'{text}'"

        node_labels = {a: n.label for a, n in self.nodes.items() if n.label}
        patterns: List[str] = []
        for edge in self.edges:
            l_label = node_labels.get(edge.left_alias)
            r_label = node_labels.get(edge.right_alias)
            left = f"({edge.left_alias}:{l_label})" if l_label else f"({edge.left_alias})"
            right = f"({edge.right_alias}:{r_label})" if r_label else f"({edge.right_alias})"
            patterns.append(f"{left}-[:{_quote_rel(edge.rel)}]->{right}")
        connected = {e.left_alias for e in self.edges} | {e.right_alias for e in self.edges}
        for alias, node in self.nodes.items():
            if alias not in connected:
                label_part = f":{node.label}" if node.label else ""
                patterns.append(f"({alias}{label_part})")
        if patterns:
            match_clause_default = "MATCH " + "\nMATCH ".join(patterns)
        else:
            match_clause_default = "MATCH"

        where_clause = ""
        if self.filters:
            rendered_filters: List[str] = []
            for flt in self.filters:
                if flt.op.upper().endswith("NULL") and flt.value is None:
                    rendered_filters.append(f"{flt.alias}.{flt.prop} {flt.op}")
                else:
                    rendered_filters.append(f"{flt.alias}.{flt.prop} {flt.op} {_format_value(flt.value)}")
            where_clause = "WHERE " + " AND ".join(rendered_filters)

        with_clause = ""
        if self.with_items:
            with_clause = "WITH " + ", ".join(self.with_items)
        with_where_clause = ""
        if self.with_filters:
            with_where_clause = "WHERE " + " AND ".join(self.with_filters)
        having_clause = ""
        if self.having_filters:
            having_clause = "HAVING " + " AND ".join(self.having_filters)

        group_clause = ""
        if self.group_by:
            group_clause = "GROUP BY " + ", ".join(self.group_by)

        return_clause = "RETURN " + ", ".join([f"{r.expr} AS {r.alias}" if r.alias else r.expr for r in self.returns])

        order_clause = ""
        if self.order_by:
            order_clause = "ORDER BY " + ", ".join([f"{o.expr} {o.direction}" for o in self.order_by])

        limit_clause = f"LIMIT {self.limit}" if isinstance(self.limit, int) and self.limit > 0 else ""

        # GraphLite compatibility: it rejects WITH ... GROUP BY ... (parser bug).
        # Workaround: when we have a single-stage aggregation with simple WITH
        # projections, drop the WITH entirely, inline those projections, and emit
        # RETURN before GROUP BY. This keeps queries valid for GraphLite without
        # changing behavior for multi-stage pipelines that genuinely need WITH.
        use_group_compat = bool(self.group_by) and bool(self.with_items) and not self.with_filters and not self.having_filters
        alias_map: Dict[str, str] = {}
        if use_group_compat:
            simple_aliases = True
            for item in self.with_items:
                m = re.match(r"(?is)\s*(.+?)\s+AS\s+([A-Za-z_][A-Za-z0-9_]*)\s*$", item)
                if m:
                    expr, alias = m.groups()
                    alias_map[alias] = expr.strip()
                else:
                    # Allow plain expressions (no alias) to pass through; they won't be expanded.
                    continue
            # Require at least one alias to rewrite; otherwise fall back.
            use_group_compat = use_group_compat and simple_aliases and bool(alias_map)

        def _expand_expr(expr: str) -> str:
            return alias_map.get(expr, expr)

        if use_group_compat:
            # Coalesce patterns into a single MATCH clause.
            match_clause = "MATCH " + ", ".join(patterns) if patterns else "MATCH"

            # Rewrite returns/group/order to inline the expressions that would have been projected in WITH.
            rewritten_returns = []
            for r in self.returns:
                if r.expr in alias_map:
                    rewritten_returns.append(IRReturn(expr=_expand_expr(r.expr), alias=r.alias or r.expr))
                else:
                    rewritten_returns.append(r)
            return_clause = "RETURN " + ", ".join([f"{r.expr} AS {r.alias}" if r.alias else r.expr for r in rewritten_returns])

            group_clause = "GROUP BY " + ", ".join([_expand_expr(g) for g in self.group_by]) if self.group_by else ""
            order_clause = (
                "ORDER BY " + ", ".join([f"{_expand_expr(o.expr)} {o.direction}" for o in self.order_by])
                if self.order_by
                else ""
            )

            parts = [match_clause]
            if where_clause:
                parts.append(where_clause)
            parts.append(return_clause)
            if group_clause:
                parts.append(group_clause)
            if order_clause:
                parts.append(order_clause)
            if limit_clause:
                parts.append(limit_clause)
            return "\n".join(parts)

        parts = [match_clause_default]
        if where_clause:
            parts.append(where_clause)
        if with_clause:
            parts.append(with_clause)
        if with_where_clause:
            parts.append(with_where_clause)
        parts.append(return_clause)
        if group_clause:
            parts.append(group_clause)
        if having_clause:
            parts.append(having_clause)
        if order_clause:
            parts.append(order_clause)
        if limit_clause:
            parts.append(limit_clause)
        return "\n".join(parts)

    def validate_bindings(self) -> List[str]:
        errors: List[str] = []
        aliases = set(self.nodes.keys())
        for edge in self.edges:
            if edge.left_alias not in aliases or edge.right_alias not in aliases:
                errors.append(f"edge references unknown alias: {edge}")
        for flt in self.filters:
            if flt.alias not in aliases:
                errors.append(f"filter alias missing: {flt.alias}")
            if isinstance(flt.value, dict):
                ref_a = flt.value.get("ref_alias")
                if ref_a and ref_a not in aliases:
                    errors.append(f"filter ref alias missing: {ref_a}")
        for ret in self.returns:
            for alias_ref in re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\.", ret.expr):
                if alias_ref not in aliases:
                    errors.append(f"return references unknown alias: {alias_ref}")
        return errors


__all__ = ["IRNode", "IREdge", "IRFilter", "IRReturn", "IROrder", "ISOQueryIR"]
