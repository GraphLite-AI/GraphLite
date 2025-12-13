from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Set, Tuple

_BINDINGS_DIR = Path(__file__).resolve().parents[2] / "bindings" / "python"
if str(_BINDINGS_DIR) not in sys.path:
    sys.path.insert(0, str(_BINDINGS_DIR))

try:  # pragma: no cover - import path varies
    from graphlite import GraphLite, GraphLiteError  # type: ignore
except Exception:  # pragma: no cover - when bindings unavailable
    GraphLite = None  # type: ignore
    GraphLiteError = Exception  # type: ignore

_ISO_DURATION_PATTERN = re.compile(
    r"^p"
    r"(?:(?P<years>[+-]?\d+(?:\.\d+)?)y)?"
    r"(?:(?P<months>[+-]?\d+(?:\.\d+)?)m)?"
    r"(?:(?P<weeks>[+-]?\d+(?:\.\d+)?)w)?"
    r"(?:(?P<days>[+-]?\d+(?:\.\d+)?)d)?"
    r"(?:t(?:(?P<hours>[+-]?\d+(?:\.\d+)?)h)?(?:(?P<minutes>[+-]?\d+(?:\.\d+)?)m)?(?:(?P<seconds>[+-]?\d+(?:\.\d+)?)s)?)?"
    r"$",
    re.IGNORECASE,
)
_MAP_DURATION_FIELD_RE = re.compile(r"(years|months|weeks|days|hours|minutes|seconds)\s*:\s*([^,}]+)", re.IGNORECASE)
_COMPARATOR_SYMBOLS = {
    "GreaterThan": ">",
    "GreaterEqual": ">=",
    "LessThan": "<",
    "LessEqual": "<=",
    "Equal": "=",
}
_FLIPPED_COMPARATORS = {
    "GreaterThan": "LessThan",
    "GreaterEqual": "LessEqual",
    "LessThan": "GreaterThan",
    "LessEqual": "GreaterEqual",
    "Equal": "Equal",
}
_SYMBOL_COMPARATOR_FLIPS = {
    ">": "<",
    ">=": "<=",
    "<": ">",
    "<=": ">=",
    "=": "=",
}
_IDENTIFIER_PATTERN = r"[A-Za-z0-9_.`]+"
_DURATION_UNIT_TOKENS = {
    "years": ("year", "years"),
    "months": ("month", "months"),
    "weeks": ("week", "weeks"),
    "days": ("day", "days"),
    "hours": ("hour", "hours"),
    "minutes": ("minute", "minutes"),
    "seconds": ("second", "seconds"),
}


@dataclass(frozen=True)
class TemporalWindow:
    unit: Optional[str]
    magnitude: Optional[str]
    magnitude_value: Optional[float]
    direction: Optional[str]
    comparator: Optional[str]
    target: Optional[str]
    source: str


@dataclass(frozen=True)
class TemporalWindowRequirement:
    direction: Optional[str] = None
    comparator: Optional[str] = None
    units: Set[str] = field(default_factory=set)
    min_magnitude: Optional[float] = None
    max_magnitude: Optional[float] = None

    def describe(self) -> str:
        pieces: List[str] = []
        if self.direction:
            pieces.append(self.direction)
        if self.units:
            pieces.append("/".join(sorted(self.units)))
        if self.comparator:
            pieces.append(self.comparator)
        if self.min_magnitude is not None or self.max_magnitude is not None:
            window: List[str] = []
            if self.min_magnitude is not None:
                window.append(f">={self.min_magnitude:g}")
            if self.max_magnitude is not None:
                window.append(f"<={self.max_magnitude:g}")
            pieces.append(" ".join(window))
        elif self.comparator:
            pieces.append("value")
        return " ".join(pieces).strip() or "temporal window"


class TemporalEvidence:
    def __init__(self, tokens: Set[str], windows: List[TemporalWindow]):
        self.tokens = {tok.lower() for tok in tokens if tok}
        self.windows = windows

    @classmethod
    def from_rendered(cls, rendered: str) -> Optional[TemporalEvidence]:
        if not rendered:
            return None
        tokens: Set[str] = set()
        windows: List[TemporalWindow] = []
        ast_available = False
        if GraphLite:
            try:
                ast = GraphLite.parse(rendered)
                ast_analyzer = _ASTTemporalAnalyzer(ast)
                ast_tokens, ast_windows = ast_analyzer.collect()
                tokens.update(ast_tokens)
                windows.extend(ast_windows)
                ast_available = True
            except GraphLiteError:
                ast_available = False
        if not ast_available:
            regex_analyzer = _RegexTemporalAnalyzer(rendered)
            reg_tokens, reg_windows = regex_analyzer.collect()
            tokens.update(reg_tokens)
            windows.extend(reg_windows)
        return cls(tokens, windows)

    def covers_token(self, token: str) -> bool:
        return token.lower() in self.tokens

    def satisfies(self, requirement: TemporalWindowRequirement) -> bool:
        normalized_direction = requirement.direction.lower() if requirement.direction else None
        normalized_units = {unit.lower() for unit in requirement.units if unit}
        normalized_comparator = requirement.comparator.lower() if requirement.comparator else None
        for window in self.windows:
            if normalized_direction and (window.direction or "") != normalized_direction:
                continue
            if normalized_units and (not window.unit or window.unit.lower() not in normalized_units):
                continue
            if normalized_comparator and (window.comparator or "").lower() != normalized_comparator:
                continue
            if requirement.min_magnitude is not None:
                if window.magnitude_value is None or window.magnitude_value < requirement.min_magnitude:
                    continue
            if requirement.max_magnitude is not None:
                if window.magnitude_value is None or window.magnitude_value > requirement.max_magnitude:
                    continue
            return True
        return False


class _ASTTemporalAnalyzer:
    def __init__(self, document: Dict[str, object]):
        self.document = document
        self.alias_map = self._collect_aliases(document)

    def collect(self) -> Tuple[Set[str], List[TemporalWindow]]:
        tokens: Set[str] = set()
        windows: List[TemporalWindow] = []
        for expr in self._iter_conditions(self.document):
            expr_tokens, expr_windows = self._gather(expr)
            tokens.update(expr_tokens)
            windows.extend(expr_windows)
        return tokens, windows

    def _iter_conditions(self, node, parent_key: Optional[str] = None):
        if isinstance(node, dict):
            if parent_key in {"where_clause", "having_clause"} and node.get("condition"):
                yield node["condition"]
            for key, value in node.items():
                yield from self._iter_conditions(value, key)
        elif isinstance(node, list):
            for item in node:
                yield from self._iter_conditions(item, parent_key)

    def _gather(self, expr) -> Tuple[Set[str], List[TemporalWindow]]:
        tokens: Set[str] = set()
        windows: List[TemporalWindow] = []
        expr_type, body = _unwrap(expr)
        if expr_type != "Binary":
            return tokens, windows
        operator = body.get("operator")
        if operator in {"And", "Or"}:
            left_tokens, left_windows = self._gather(body.get("left"))
            right_tokens, right_windows = self._gather(body.get("right"))
            tokens.update(left_tokens)
            tokens.update(right_tokens)
            windows.extend(left_windows)
            windows.extend(right_windows)
            return tokens, windows
        for window in self._windows_from_binary(body):
            tokens.update(_tokens_for_window(window))
            windows.append(window)
        return tokens, windows

    def _windows_from_binary(self, binary: Dict[str, object]) -> List[TemporalWindow]:
        comparator = binary.get("operator")
        if comparator not in _COMPARATOR_SYMBOLS:
            return []
        left = binary.get("left")
        right = binary.get("right")
        prop = self._property(left)
        expr = right
        if not prop:
            prop = self._property(right)
            if not prop:
                return []
            expr = left
            comparator = _FLIPPED_COMPARATORS.get(comparator)
            if comparator not in _COMPARATOR_SYMBOLS:
                return []
        expr = self._resolve_alias(expr)
        windows: List[TemporalWindow] = []
        for unit, magnitude, numeric, direction in self._duration_windows(expr):
            windows.append(
                TemporalWindow(
                    unit=unit,
                    magnitude=magnitude,
                    magnitude_value=numeric,
                    direction=direction,
                    comparator=_COMPARATOR_SYMBOLS.get(comparator),
                    target=f"{prop[0]}.{prop[1]}",
                    source="ast",
                )
            )
        return windows

    def _duration_windows(self, expr) -> List[Tuple[Optional[str], Optional[str], Optional[float], Optional[str]]]:
        expr = self._resolve_alias(expr)
        expr_type, body = _unwrap(expr)
        if expr_type == "Binary" and body.get("operator") in {"Minus", "Plus"}:
            direction = "past" if body["operator"] == "Minus" else "future"
            duration = self._duration_parts(body.get("right"))
            return [
                (unit, magnitude or None, numeric, direction)
                for unit, (magnitude, numeric) in duration.items()
            ]
        return []

    def _duration_parts(self, expr) -> Dict[str, Tuple[str, Optional[float]]]:
        expr = self._resolve_alias(expr)
        expr_type, body = _unwrap(expr)
        if expr_type == "FunctionCall" and body.get("name", "").upper() == "DURATION":
            args = body.get("arguments") or []
            if not args:
                return {}
            literal = self._literal_value(args[0])
            if isinstance(literal, str):
                return _parse_iso_duration(literal)
        if expr_type == "Literal":
            literal = self._literal_value(expr)
            if isinstance(literal, str):
                return _parse_iso_duration(literal)
        return {}

    def _property(self, expr) -> Optional[Tuple[str, str]]:
        expr_type, body = _unwrap(expr)
        if expr_type == "PropertyAccess":
            return body.get("object"), body.get("property")
        return None

    def _resolve_alias(self, expr, seen: Optional[Set[str]] = None):
        expr_type, body = _unwrap(expr)
        if expr_type == "Variable":
            alias = (body.get("name") or "").lower()
            if alias in self.alias_map:
                seen = set(seen or set())
                if alias in seen:
                    return expr
                seen.add(alias)
                return self._resolve_alias(self.alias_map[alias], seen)
        return expr

    def _literal_value(self, expr) -> Optional[str]:
        expr_type, body = _unwrap(expr)
        if expr_type != "Literal" or not isinstance(body, dict):
            return None
        for key in ("String", "Duration"):
            if key in body:
                return body[key]
        return None

    def _collect_aliases(self, node) -> Dict[str, object]:
        mapping: Dict[str, object] = {}
        self._traverse_aliases(node, mapping)
        return mapping

    def _traverse_aliases(self, node, mapping: Dict[str, object]):
        if isinstance(node, dict):
            clause = node.get("with_clause")
            if clause:
                for item in clause.get("items", []):
                    alias = item.get("alias")
                    expr = item.get("expression")
                    if alias and expr:
                        mapping[alias.lower()] = expr
            for value in node.values():
                self._traverse_aliases(value, mapping)
        elif isinstance(node, list):
            for item in node:
                self._traverse_aliases(item, mapping)


class _RegexTemporalAnalyzer:
    _DURATION_CALL_RE = re.compile(r"duration\s*\((?P<body>[^)]*)\)", re.IGNORECASE)
    _LEFT_COMPARATOR_RE = re.compile(rf"(?P<target>{_IDENTIFIER_PATTERN})\s*[\)\]]*\s*(?P<op>>=|<=|>|<|=)", re.IGNORECASE)
    _RIGHT_COMPARATOR_RE = re.compile(rf"^\s*(?P<op>>=|<=|>|<|=)\s*[\(\[]*(?P<target>{_IDENTIFIER_PATTERN})", re.IGNORECASE)

    def __init__(self, text: str):
        self.text = text

    def collect(self) -> Tuple[Set[str], List[TemporalWindow]]:
        tokens: Set[str] = set()
        windows: List[TemporalWindow] = []
        for match in self._DURATION_CALL_RE.finditer(self.text):
            body = match.group("body")
            parts = _parse_map_duration(body) or _parse_inline_duration(body)
            if not parts:
                continue
            direction = self._infer_direction(match.start())
            comparator, target, context_token = self._find_comparator(match.start(), match.end())
            tokens.update(_tokens_for_parts(parts))
            tokens.update(_qualifier_tokens(direction, comparator))
            if context_token:
                tokens.add(context_token)
            built = self._build_windows(parts, direction, comparator, target)
            if built:
                windows.extend(built)
            else:
                windows.append(
                    TemporalWindow(
                        unit=None,
                        magnitude=None,
                        magnitude_value=None,
                        direction=direction,
                        comparator=None,
                        target=None,
                        source="regex",
                    )
                )
        return tokens, windows

    def _infer_direction(self, start: int) -> Optional[str]:
        idx = start - 1
        while idx >= 0:
            ch = self.text[idx]
            if ch.isspace():
                idx -= 1
                continue
            if ch == "-":
                return "past"
            if ch == "+":
                return "future"
            break
        return None

    def _build_windows(
        self,
        parts: Dict[str, Tuple[str, Optional[float]]],
        direction: Optional[str],
        comparator: Optional[str],
        target: Optional[str],
    ) -> List[TemporalWindow]:
        windows: List[TemporalWindow] = []
        for unit, (magnitude, numeric) in parts.items():
            windows.append(
                TemporalWindow(
                    unit=unit,
                    magnitude=magnitude,
                    magnitude_value=numeric,
                    direction=direction,
                    comparator=comparator,
                    target=target,
                    source="regex",
                )
            )
        return windows

    def _find_comparator(self, start: int, end: int) -> Tuple[Optional[str], Optional[str], Optional[str]]:
        comparator, target = self._comparator_from_left(start)
        if comparator:
            return comparator, target, None
        comparator, target = self._comparator_from_right(end)
        if comparator:
            return comparator, target, None
        comparator, target = self._comparator_from_between(start)
        if comparator and target:
            return comparator, target, "between"
        return None, None, None

    def _comparator_from_left(self, start: int) -> Tuple[Optional[str], Optional[str]]:
        span_start = max(0, start - 160)
        context = self.text[span_start:start]
        matches = list(self._LEFT_COMPARATOR_RE.finditer(context))
        if not matches:
            return None, None
        last = matches[-1]
        return last.group("op"), last.group("target")

    def _comparator_from_right(self, end: int) -> Tuple[Optional[str], Optional[str]]:
        context = self.text[end : min(len(self.text), end + 160)]
        match = self._RIGHT_COMPARATOR_RE.match(context)
        if not match:
            return None, None
        op = match.group("op")
        flipped = _SYMBOL_COMPARATOR_FLIPS.get(op, op)
        return flipped, match.group("target")

    def _comparator_from_between(self, start: int) -> Tuple[Optional[str], Optional[str]]:
        lowered = self.text.lower()
        between_idx = lowered.rfind("between", 0, start)
        if between_idx == -1:
            return None, None
        target = self._between_target(between_idx)
        if not target:
            return None, None
        segment = self.text[between_idx:start]
        if re.search(r"\band\b", segment, re.IGNORECASE):
            return "<=", target
        return ">=", target

    def _between_target(self, between_idx: int) -> Optional[str]:
        lookback = self.text[max(0, between_idx - 120) : between_idx].rstrip()
        while lookback.endswith(")") or lookback.endswith("]"):
            lookback = lookback[:-1].rstrip()
        match = re.search(rf"(?P<target>{_IDENTIFIER_PATTERN})$", lookback)
        if match:
            return match.group("target")
        return None


def _parse_inline_duration(body: str) -> Dict[str, Tuple[str, Optional[float]]]:
    body = body.strip()
    if (body.startswith("'") and body.endswith("'")) or (body.startswith('"') and body.endswith('"')):
        return _parse_iso_duration(body.strip('"\''))
    return {}


def _parse_iso_duration(value: str) -> Dict[str, Tuple[str, Optional[float]]]:
    match = _ISO_DURATION_PATTERN.fullmatch(value.strip().lower())
    if not match:
        return {}
    parts: Dict[str, Tuple[str, Optional[float]]] = {}
    for key, raw in match.groupdict().items():
        if not raw:
            continue
        normalized, numeric = _parse_numeric_value(raw)
        if numeric == 0:
            continue
        parts[key] = (normalized, numeric)
    return parts


def _parse_map_duration(body: str) -> Dict[str, Tuple[str, Optional[float]]]:
    if not body.strip().startswith("{"):
        return {}
    parts: Dict[str, Tuple[str, Optional[float]]] = {}
    for key, raw in _MAP_DURATION_FIELD_RE.findall(body):
        normalized, numeric = _parse_numeric_value(raw)
        if numeric == 0:
            continue
        parts[key.lower()] = (normalized, numeric)
    return parts


def _parse_numeric_value(raw: str) -> Tuple[str, Optional[float]]:
    cleaned = raw.strip().lstrip("+")
    try:
        numeric = float(cleaned)
    except ValueError:
        return cleaned, None
    if numeric.is_integer():
        normalized = str(int(numeric))
    else:
        normalized = ("%f" % numeric).rstrip("0").rstrip(".")
    return normalized, numeric


def _tokens_for_parts(parts: Dict[str, Tuple[str, Optional[float]]]) -> Set[str]:
    tokens: Set[str] = set()
    for unit, value in parts.items():
        names = _DURATION_UNIT_TOKENS.get(unit)
        if not names:
            continue
        singular, plural = names
        tokens.update({singular, plural})
        raw_value = value[0]
        if raw_value:
            normalized = raw_value.lower()
            tokens.add(normalized)
            tokens.add(f"{normalized} {singular}")
            tokens.add(f"{normalized} {plural}")
    return tokens


def _qualifier_tokens(direction: Optional[str], comparator: Optional[str]) -> Set[str]:
    tokens: Set[str] = set()
    if direction == "past":
        tokens.update({"past", "ago", "last", "within"})
    elif direction == "future":
        tokens.update({"future", "next", "upcoming", "within"})
    return tokens


def _tokens_for_window(window: TemporalWindow) -> Set[str]:
    tokens: Set[str] = set()
    if window.unit and window.unit in _DURATION_UNIT_TOKENS:
        singular, plural = _DURATION_UNIT_TOKENS[window.unit]
        tokens.update({singular, plural})
    if window.magnitude:
        normalized = window.magnitude.lower()
        tokens.add(normalized)
        if window.unit and window.unit in _DURATION_UNIT_TOKENS:
            singular, plural = _DURATION_UNIT_TOKENS[window.unit]
            tokens.add(f"{normalized} {singular}")
            tokens.add(f"{normalized} {plural}")
    if window.direction == "past":
        tokens.update({"past", "ago", "last"})
    elif window.direction == "future":
        tokens.update({"future", "next", "upcoming"})
    if window.unit:
        tokens.add("duration")
    return {tok for tok in tokens if tok}


def _unwrap(expr) -> Tuple[Optional[str], Dict[str, object]]:
    if not isinstance(expr, dict) or len(expr) != 1:
        return None, {}
    key = next(iter(expr))
    return key, expr[key]


__all__ = ["TemporalEvidence", "TemporalWindow", "TemporalWindowRequirement"]
