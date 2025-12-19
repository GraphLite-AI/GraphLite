from __future__ import annotations

import re
from typing import Iterable, List, Set, Tuple

from .ir import ISOQueryIR


Identifier = str


def _aliases_in_expr(expr: str) -> Set[Identifier]:
    """
    Extract aliases and bare identifiers referenced in an expression.
    - token before a dot is treated as an alias reference
    - bare identifiers are collected when they are not obviously function calls
    """
    if not expr:
        return set()
    aliases: Set[str] = set()
    prop_tokens: Set[str] = set()
    for alias, prop in re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)", expr):
        aliases.add(alias)
        prop_tokens.add(prop)
    reserved = {
        "match",
        "where",
        "return",
        "with",
        "group",
        "order",
        "limit",
        "asc",
        "desc",
        "and",
        "or",
        "not",
        "count",
        "sum",
        "avg",
        "average",
        "min",
        "max",
        "collect",
        "distinct",
        "having",
    }
    tokens = re.findall(r"([A-Za-z_][A-Za-z0-9_]*)", expr)
    for tok in tokens:
        lower = tok.lower()
        if lower in reserved:
            continue
        if re.search(rf"\b{re.escape(tok)}\s*\(", expr):
            # Likely a function call; skip as a reference.
            continue
        if tok in prop_tokens:
            continue
        aliases.add(tok)
    return aliases


def _produced_aliases_from_with(items: Iterable[str]) -> Tuple[Set[Identifier], List[str]]:
    produced: Set[str] = set()
    errors: List[str] = []
    for item in items:
        if not item:
            continue
        segments = [seg.strip() for seg in item.split(",") if seg.strip()]
        for seg in segments:
            if " AS " in seg.upper():
                parts = re.split(r"\s+AS\s+", seg, flags=re.IGNORECASE)
                if len(parts) == 2:
                    alias = parts[1].strip()
                    if "." in alias:
                        errors.append(f"invalid identifier in WITH alias: {alias}")
                        continue
                    produced.add(alias)
                    continue
            if re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", seg):
                produced.add(seg)
    return produced, errors


def validate_structure(query: str, ir: ISOQueryIR) -> List[str]:
    """
    Lightweight structural checks beyond parsing:
    - No illegal identifiers (e.g., dots in aliases produced by WITH/RETURN).
    - Scope propagation: RETURN/ORDER/with_filters may only reference aliases
      introduced by MATCH (and carried through WITH when present).
    - WITH expressions may only reference aliases that existed before the WITH.
    """
    errors: List[str] = []
    query_text = query.strip()
    if not query_text:
        return ["empty query"]

    # Base scope from MATCH aliases.
    base_scope: Set[str] = set(ir.nodes.keys())
    if not base_scope:
        errors.append("no aliases produced by MATCH")

    # WITH scope
    with_scope, with_alias_errors = _produced_aliases_from_with(ir.with_items)
    errors.extend(with_alias_errors)

    scope_before_with = set(base_scope)
    scope_after_with = set(with_scope) if ir.with_items else set(base_scope)

    # WITH expressions must reference previous scope only.
    for item in ir.with_items:
        expr = item
        if " AS " in item.upper():
            expr = re.split(r"\s+AS\s+", item, flags=re.IGNORECASE)[0]
        refs = _aliases_in_expr(expr)
        missing = refs - scope_before_with
        if missing:
            errors.append(f"WITH references unknown aliases: {', '.join(sorted(missing))}")

    # Filters before WITH use base scope; filters after WITH (with_filters) use WITH scope.
    for flt in ir.filters:
        if flt.alias not in base_scope:
            errors.append(f"WHERE references unknown alias: {flt.alias}")

    for fexpr in ir.with_filters:
        refs = _aliases_in_expr(fexpr)
        missing = refs - scope_after_with
        if missing:
            errors.append(f"post-WITH filter references unknown aliases: {', '.join(sorted(missing))}")

    # RETURN scope
    return_scope = scope_after_with
    for ret in ir.returns:
        refs = _aliases_in_expr(ret.expr)
        missing = refs - return_scope
        if missing:
            errors.append(f"RETURN references unknown aliases: {', '.join(sorted(missing))}")
        if ret.alias and "." in ret.alias:
            errors.append(f"invalid RETURN alias identifier: {ret.alias}")

    for having_expr in ir.having_filters:
        refs = _aliases_in_expr(having_expr)
        missing = refs - return_scope
        if missing:
            errors.append(f"HAVING references unknown aliases: {', '.join(sorted(missing))}")

    # ORDER BY scope
    for order in ir.order_by:
        refs = _aliases_in_expr(order.expr)
        missing = refs - return_scope
        if missing:
            errors.append(f"ORDER BY references unknown aliases: {', '.join(sorted(missing))}")

    return sorted(set(errors))


__all__ = ["validate_structure"]
