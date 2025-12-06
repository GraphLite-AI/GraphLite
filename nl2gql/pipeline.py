"""Research-grade, schema-agnostic NL → ISO GQL pipeline.

Architecture:
1) Preprocessor: normalizes NL, extracts entity-like phrases, filters schema via
   exact, ner-masked exact, and semantic similarity strategies, then surfaces
   adjacency-based path hints (schema-agnostic).
2) Generator: prompt-scaffolds gpt-4o-mini to emit one or more ISO GQL drafts
   constrained by the filtered schema and structural hints.
3) Refiner: validates syntax, schema grounding, logic, and execution feedback;
   iteratively repairs (max 3 loops) using deterministic feedback fusion.

Additional layers:
- SchemaGraph abstraction with adjacency, property listings, distance/path
  queries, and semantic matching utilities.
- ISO GQL IR with deterministic parse/render used for validation/repair.
- Extensible validation stack: syntax (GraphLite), schema grounding, logic
  (LLM), execution-aware evaluation.
- Sample-suite runner that executes every query in sample_queries.txt.
"""

from __future__ import annotations

import argparse
import difflib
import json
import os
import re
import sys
import tempfile
import threading
import time
from collections import defaultdict, deque
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, List, Optional, Sequence, Set, Tuple

from tenacity import retry, stop_after_attempt, wait_fixed

try:
    from dotenv import load_dotenv
except ImportError:  # pragma: no cover
    load_dotenv = None  # type: ignore

# -----------------------------------------------------------------------------
# GraphLite SDK loader
# -----------------------------------------------------------------------------


def _load_graphlite_sdk():
    """Import GraphLite bindings, preferring the bindings/python package."""

    try:
        from graphlite import GraphLite, GraphLiteError  # type: ignore

        if getattr(GraphLite, "__name__", None):
            return GraphLite, GraphLiteError
    except Exception:
        pass

    bindings_path = Path(__file__).resolve().parents[1] / "bindings" / "python"
    if bindings_path.exists():
        sys.path.insert(0, str(bindings_path))
        sys.modules.pop("graphlite", None)
        from graphlite import GraphLite, GraphLiteError  # type: ignore

        return GraphLite, GraphLiteError

    raise SystemExit(
        "GraphLite Python bindings are missing. Build the FFI and install with: "
        "cargo build -p graphlite-ffi --release && pip install -e bindings/python"
    )


GraphLite, GraphLiteError = _load_graphlite_sdk()

try:
    from openai import OpenAI
except Exception as exc:  # pragma: no cover
    raise SystemExit("OpenAI client missing. Install with: pip install openai") from exc

# -----------------------------------------------------------------------------
# Environment + OpenAI helpers
# -----------------------------------------------------------------------------


_ENV_PATH = Path(__file__).with_name("config.env")
if load_dotenv:
    if _ENV_PATH.exists():
        load_dotenv(_ENV_PATH)
    else:
        load_dotenv()

DEFAULT_OPENAI_MODEL_GEN = os.getenv("OPENAI_MODEL_GEN", "gpt-4o-mini")
DEFAULT_OPENAI_MODEL_FIX = os.getenv("OPENAI_MODEL_FIX", "gpt-4o-mini")

DEFAULT_DB_PATH = os.getenv("NL2GQL_DB_PATH")
DEFAULT_DB_USER = os.getenv("NL2GQL_USER", "admin")
DEFAULT_DB_SCHEMA = os.getenv("NL2GQL_SCHEMA", "nl2gql")
DEFAULT_DB_GRAPH = os.getenv("NL2GQL_GRAPH", "scratch")

_client_singleton: Optional[OpenAI] = None
_USAGE_LOG: List[Dict[str, int]] = []


def _client() -> OpenAI:
    global _client_singleton
    if _client_singleton is None:
        _client_singleton = OpenAI()
    return _client_singleton


def _reset_usage_log() -> None:
    _USAGE_LOG.clear()


def _record_usage(usage: Dict[str, Any]) -> None:
    prompt = int(usage.get("prompt_tokens", 0))
    completion = int(usage.get("completion_tokens", 0))
    total = int(usage.get("total_tokens", prompt + completion))
    _USAGE_LOG.append({"prompt_tokens": prompt, "completion_tokens": completion, "total_tokens": total})


def _usage_totals() -> Dict[str, int]:
    totals = {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
    for entry in _USAGE_LOG:
        totals["prompt_tokens"] += int(entry.get("prompt_tokens", 0))
        totals["completion_tokens"] += int(entry.get("completion_tokens", 0))
        totals["total_tokens"] += int(entry.get("total_tokens", 0))
    return totals


@retry(stop=stop_after_attempt(3), wait=wait_fixed(0.25))
def chat_complete(
    model: str,
    system: str,
    user: str,
    *,
    temperature: float = 0.15,
    top_p: float = 0.9,
    max_tokens: int = 700,
) -> Tuple[str, Optional[Dict[str, Any]]]:
    """Chat completion with retry + usage extraction."""

    resp = _client().chat.completions.create(
        model=model,
        messages=[{"role": "system", "content": system}, {"role": "user", "content": user}],
        temperature=temperature,
        top_p=top_p,
        max_tokens=max_tokens,
    )

    text = (resp.choices[0].message.content or "").strip()
    usage_data = getattr(resp, "usage", None)
    if usage_data:
        usage = {
            "prompt_tokens": getattr(usage_data, "prompt_tokens", 0),
            "completion_tokens": getattr(usage_data, "completion_tokens", 0),
            "total_tokens": getattr(usage_data, "total_tokens", 0),
        }
        _record_usage(usage)
        return text, usage
    return text, None


def _clean_block(text: str) -> str:
    stripped = text.strip()
    if stripped.startswith("```"):
        stripped = stripped[stripped.find("\n") + 1 :] if "\n" in stripped else stripped.lstrip("`")
    if stripped.endswith("```"):
        stripped = stripped[: stripped.rfind("```")]
    return stripped.strip()


def _safe_json_loads(text: str) -> Optional[Any]:
    try:
        return json.loads(_clean_block(text))
    except Exception:
        return None


# -----------------------------------------------------------------------------
# UI helpers
# -----------------------------------------------------------------------------


_ANSI_COLORS = {
    "mauve": "\033[38;5;141m",
    "peach": "\033[38;5;209m",
    "sky": "\033[38;5;117m",
    "teal": "\033[38;5;37m",
    "blue": "\033[34m",
    "white": "\033[37m",
    "green": "\033[32m",
    "red": "\033[31m",
    "reset": "\033[0m",
    "italic": "\033[3m",
}


def _style(text: str, color: str, enabled: bool, *, italic: bool = False) -> str:
    if not enabled:
        return text
    code = _ANSI_COLORS.get("blue" if italic else color, "")
    italic_code = _ANSI_COLORS["italic"] if italic else ""
    if not (code or italic_code):
        return text
    return f"{italic_code}{code}{text}{_ANSI_COLORS['reset']}"


class Spinner:
    """Lightweight terminal spinner for live status updates."""

    def __init__(self, enabled: bool = True, color: str = "mauve") -> None:
        self.enabled = enabled and sys.stdout.isatty()
        self.color = color
        self._text = ""
        self._parts: Optional[Tuple[int, str]] = None
        self._stop = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._last_len = 0

    def start(self, initial: str = "") -> None:
        self._text = initial
        self._parts = self._split_attempt(initial)
        if not self.enabled:
            return
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def update(self, text: str) -> None:
        self._text = text
        self._parts = self._split_attempt(text)

    def stop(self, final: Optional[str] = None, color: Optional[str] = None) -> None:
        if self.enabled:
            self._stop.set()
            if self._thread:
                self._thread.join(timeout=0.5)
            sys.stdout.write("\r" + " " * self._last_len + "\r")
            sys.stdout.flush()
        if final:
            if self.enabled and color:
                print(_style(final, color, True))
            elif self.enabled:
                print(_style(final, "green", True))
            else:
                print(final)

    @staticmethod
    def _split_attempt(text: str) -> Optional[Tuple[int, str]]:
        if text.startswith("[attempt") and "]" in text:
            end = text.find("]") + 1
            prefix = text[:end]
            rest = text[end:].lstrip()
            try:
                num_part = prefix.strip("[]").split()[1]
                num = int(num_part)
            except Exception:
                return None
            return num, rest
        return None

    def _run(self) -> None:
        frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
        idx = 0
        while not self._stop.is_set():
            frame = frames[idx % len(frames)]
            frame_col = _style(frame, self.color, self.enabled)
            if self._parts:
                _, rest = self._parts
                rest_col = _style(rest, "white", self.enabled, italic=True)
                text_col = rest_col
            else:
                text_col = _style(self._text, "white", self.enabled, italic=True)
            line = f"\r{frame_col} {text_col}"
            self._last_len = max(self._last_len, len(line))
            sys.stdout.write(line + " " * max(0, self._last_len - len(line)))
            sys.stdout.flush()
            idx += 1
            time.sleep(0.08)


# -----------------------------------------------------------------------------
# Schema graph layer
# -----------------------------------------------------------------------------


def _canonical(text: str) -> str:
    return re.sub(r"[^a-z0-9]", "", text.lower())


def _tokenize(text: str) -> List[str]:
    return re.findall(r"[a-zA-Z][a-zA-Z0-9_]*", text)


def _ratio(a: str, b: str) -> float:
    if not a or not b:
        return 0.0
    return difflib.SequenceMatcher(None, a, b).ratio()


@dataclass
class SchemaNode:
    name: str
    properties: List[str] = field(default_factory=list)


@dataclass
class SchemaEdge:
    src: str
    rel: str
    dst: str

    def descriptor(self) -> str:
        return f"({self.src})-[:{self.rel}]->({self.dst})"


@dataclass
class SchemaGraph:
    nodes: Dict[str, SchemaNode]
    edges: List[SchemaEdge]
    adjacency: Dict[str, List[SchemaEdge]] = field(default_factory=dict)

    @classmethod
    def from_text(cls, schema_context: str) -> "SchemaGraph":
        nodes: Dict[str, SchemaNode] = {}
        edges: List[SchemaEdge] = []
        for raw in schema_context.splitlines():
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("- "):
                line = line[2:].strip()
            if line.startswith("* "):
                line = line[2:].strip()
            rel_match = re.match(r"^\(?([A-Za-z0-9_]+)\)?-?\s*\[:([A-Za-z0-9_]+)\]\s*->\s*\(?([A-Za-z0-9_]+)\)?", line)
            if rel_match:
                edges.append(SchemaEdge(rel_match.group(1), rel_match.group(2), rel_match.group(3)))
                continue
            ent_match = re.match(r"^-?\s*([A-Za-z0-9_]+)\s*:\s*(.+)$", line)
            if ent_match:
                name = ent_match.group(1).strip()
                props_text = ent_match.group(2)
                props = [p.strip() for p in re.split(r"[;,]", props_text) if p.strip()]
                node = nodes.get(name) or SchemaNode(name=name)
                node.properties = sorted(set(node.properties + props))
                nodes[name] = node
                continue
        adjacency: Dict[str, List[SchemaEdge]] = defaultdict(list)
        for edge in edges:
            adjacency[edge.src].append(edge)
            adjacency[edge.dst]  # ensure target is keyed
        return cls(nodes=nodes, edges=edges, adjacency=dict(adjacency))

    def list_properties(self, node: str) -> List[str]:
        return self.nodes.get(node, SchemaNode(node, [])).properties

    def has_node(self, name: str) -> bool:
        return name in self.nodes

    def has_property(self, node: str, prop: str) -> bool:
        return prop in self.list_properties(node)

    def edge_exists(self, left: str, rel: str, right: str) -> bool:
        return any(e.src == left and e.rel == rel and e.dst == right for e in self.edges)

    def neighbors(self, node: str, *, include_inbound: bool = True) -> List[SchemaEdge]:
        edges = list(self.adjacency.get(node, []))
        if include_inbound:
            for e in self.edges:
                if e.dst == node:
                    edges.append(SchemaEdge(src=node, rel=e.rel, dst=e.src))
        return edges

    def semantic_matches(self, term: str, pool: Iterable[str], limit: int = 3) -> List[Tuple[str, float]]:
        scored = []
        canon_term = _canonical(term)
        for item in pool:
            score = _ratio(canon_term, _canonical(item))
            if score > 0.0:
                scored.append((item, score))
        scored.sort(key=lambda x: x[1], reverse=True)
        return scored[:limit]

    def shortest_paths(self, starts: Set[str], targets: Set[str], max_depth: int = 3) -> List[List[SchemaEdge]]:
        if not starts or not targets:
            return []
        paths: List[List[SchemaEdge]] = []
        for start in starts:
            queue: deque[Tuple[str, List[SchemaEdge]]] = deque()
            queue.append((start, []))
            visited: Set[str] = set()
            while queue:
                node, path = queue.popleft()
                if len(path) > max_depth:
                    continue
                if node in visited:
                    continue
                visited.add(node)
                if node in targets and path:
                    paths.append(path)
                for edge in self.neighbors(node):
                    queue.append((edge.dst, path + [edge]))
        return paths

    @staticmethod
    def describe_subset(nodes: Dict[str, SchemaNode], edges: List[SchemaEdge]) -> str:
        node_lines = [f"- {n.name}: {', '.join(n.properties) if n.properties else 'no properties listed'}" for n in nodes.values()]
        edge_lines = [f"- {e.descriptor()}" for e in edges]
        return "ENTITIES:\n" + "\n".join(node_lines) + "\nRELATIONSHIPS:\n" + "\n".join(edge_lines)

    def describe_full(self) -> str:
        return self.describe_subset(self.nodes, self.edges)


@dataclass
class FilteredSchema:
    nodes: Dict[str, SchemaNode]
    edges: List[SchemaEdge]
    strategy_hits: Dict[str, List[str]]
    path_hints: List[str]

    def describe(self) -> str:
        return SchemaGraph.describe_subset(self.nodes, self.edges)

    def summary_lines(self) -> str:
        node_lines = [f"- {n.name}: {', '.join(n.properties) if n.properties else 'no properties'}" for n in self.nodes.values()]
        edge_lines = [f"- {e.descriptor()}" for e in self.edges]
        hint_lines = [f"- {h}" for h in self.path_hints] if self.path_hints else ["- none"]
        return "\n".join(["Entities:"] + node_lines + ["Relationships:"] + edge_lines + ["Path hints:"] + hint_lines)


@dataclass
class PreprocessResult:
    raw_nl: str
    normalized_nl: str
    phrases: List[str]
    filtered_schema: FilteredSchema
    structural_hints: List[str]


@dataclass
class IntentLinkGuidance:
    frame: Dict[str, Any]
    links: Dict[str, Any]


# -----------------------------------------------------------------------------
# ISO GQL IR
# -----------------------------------------------------------------------------


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
    returns: List[IRReturn] = field(default_factory=list)
    order_by: List[IROrder] = field(default_factory=list)
    limit: Optional[int] = None

    @classmethod
    def parse(cls, query: str) -> Tuple[Optional["ISOQueryIR"], List[str]]:
        errors: List[str] = []
        text = query.strip()
        if not text:
            return None, ["empty query"]

        token_pattern = re.compile(r"\bMATCH\b|\bWHERE\b|\bWITH\b|\bRETURN\b|\bORDER\s+BY\b|\bLIMIT\b", flags=re.IGNORECASE)
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
        with_where_block, _ = _block("WHERE", after=with_tok["start"]) if with_tok else ("", None)
        return_block, return_tok = _block("RETURN", after=match_end)
        order_block, order_tok = _block("ORDER BY", after=return_tok["start"] if return_tok else match_end)
        limit_block = ""
        if return_tok:
            limit_block, _ = _block("LIMIT", after=order_tok["start"] if order_tok else return_tok["start"])

        nodes: Dict[str, IRNode] = {}
        edges: List[IREdge] = []
        filters: List[IRFilter] = []

        if match_blocks:
            def _parse_value(val_raw: str) -> Any:
                val_raw = val_raw.strip()
                if val_raw.lower() in {"true", "false"}:
                    return val_raw.lower() == "true"
                if re.match(r"^-?\d+(\.\d+)?$", val_raw):
                    return float(val_raw) if "." in val_raw else int(val_raw)
                if val_raw.startswith("'") and val_raw.endswith("'"):
                    return val_raw.strip("'").replace("\\'", "'")
                return val_raw

            node_pattern = re.compile(
                r"\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*([A-Za-z0-9_]+))?\s*(?:\{([^}]*)\})?\s*\)"
            )
            edge_forward = re.compile(
                r"(?=(\(\s*(?P<src>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<src_label>[A-Za-z0-9_]+))?\s*(?:\{[^}]*\})?\s*\)"
                r"\s*-\s*\[:\s*(?P<rel>[A-Za-z0-9_]+)\s*\]\s*->\s*"
                r"\(\s*(?P<dst>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<dst_label>[A-Za-z0-9_]+))?\s*(?:\{[^}]*\})?\s*\)))"
            )
            edge_backward = re.compile(
                r"(?=(\(\s*(?P<left>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<left_label>[A-Za-z0-9_]+))?\s*(?:\{[^}]*\})?\s*\)"
                r"\s*<-\s*\[:\s*(?P<rel>[A-Za-z0-9_]+)\s*\]\s*-\s*"
                r"\(\s*(?P<right>[A-Za-z_][A-Za-z0-9_]*)\s*(?::\s*(?P<right_label>[A-Za-z0-9_]+))?\s*(?:\{[^}]*\})?\s*\)))"
            )

            seen_edges: Set[Tuple[str, str, str]] = set()

            for match_block, _ in match_blocks:
                for alias, label, props in node_pattern.findall(match_block):
                    existing = nodes.get(alias)
                    if not (existing and existing.label):
                        nodes[alias] = IRNode(alias=alias, label=label or (existing.label if existing else None))
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
                    # Direction is right -> left because of "<-"
                    src, dst = right, left
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
                null_match = re.match(
                    r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)\s+IS\s+(NOT\s+)?NULL", clause, flags=re.IGNORECASE
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
                elif null_match:
                    alias, prop, not_part = null_match.group(1), null_match.group(2), null_match.group(3)
                    op = "IS NOT NULL" if not_part else "IS NULL"
                    filters.append(IRFilter(alias=alias, prop=prop, op=op, value=None))
                else:
                    errors.append(f"unparsed WHERE clause: {clause}")

        with_items: List[str] = []
        with_filters: List[str] = []
        if with_block:
            with_items = [item.strip() for item in with_block.split(",") if item.strip()]
        if with_where_block:
            with_filters = [c.strip() for c in re.split(r"\bAND\b", with_where_block, flags=re.IGNORECASE) if c.strip()]

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

        limit: Optional[int] = int(limit_block) if limit_block.isdigit() else None

        # If WITH appeared after RETURN in the original text, treat RETURN expressions as the aggregation stage
        # and keep a final RETURN of the aggregated aliases.
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
                returns=returns,
                order_by=order_by,
                limit=limit,
            ),
            errors,
        )

    def render(self) -> str:
        def _format_value(val: Any) -> str:
            if isinstance(val, bool):
                return "true" if val else "false"
            if isinstance(val, (int, float)):
                return str(val)
            if isinstance(val, dict) and "ref_alias" in val and "ref_property" in val:
                return f"{val['ref_alias']}.{val['ref_property']}"
            text = str(val)
            text = text.replace("\\", "\\\\").replace("'", "\\'")
            return f"'{text}'"

        node_labels = {a: n.label for a, n in self.nodes.items() if n.label}
        patterns: List[str] = []
        for edge in self.edges:
            l_label = node_labels.get(edge.left_alias)
            r_label = node_labels.get(edge.right_alias)
            left = f"({edge.left_alias}:{l_label})" if l_label else f"({edge.left_alias})"
            right = f"({edge.right_alias}:{r_label})" if r_label else f"({edge.right_alias})"
            patterns.append(f"{left}-[:{edge.rel}]->{right}")
        connected = {e.left_alias for e in self.edges} | {e.right_alias for e in self.edges}
        for alias, node in self.nodes.items():
            if alias not in connected:
                label_part = f":{node.label}" if node.label else ""
                patterns.append(f"({alias}{label_part})")
        match_clause = "MATCH " + ", ".join(patterns)

        where_clause = ""
        if self.filters:
            rendered_filters: List[str] = []
            for flt in self.filters:
                if flt.op.upper().endswith("NULL") and flt.value is None:
                    rendered_filters.append(f"{flt.alias}.{flt.prop} {flt.op}")
                else:
                    rendered_filters.append(f"{flt.alias}.{flt.prop} {flt.op} {_format_value(flt.value)}")
            parts = rendered_filters
            where_clause = "WHERE " + " AND ".join(parts)

        with_clause = ""
        if self.with_items:
            with_clause = "WITH " + ", ".join(self.with_items)
        with_where_clause = ""
        if self.with_filters:
            with_where_clause = "WHERE " + " AND ".join(self.with_filters)

        return_clause = "RETURN " + ", ".join(
            [f"{r.expr} AS {r.alias}" if r.alias else r.expr for r in self.returns]
        )

        order_clause = ""
        if self.order_by:
            order_clause = "ORDER BY " + ", ".join([f"{o.expr} {o.direction}" for o in self.order_by])

        limit_clause = f"LIMIT {self.limit}" if isinstance(self.limit, int) and self.limit > 0 else ""

        parts = [match_clause]
        if where_clause:
            parts.append(where_clause)
        if with_clause:
            parts.append(with_clause)
        if with_where_clause:
            parts.append(with_where_clause)
        parts.append(return_clause)
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


# -----------------------------------------------------------------------------
# Preprocessor
# -----------------------------------------------------------------------------


class Preprocessor:
    def __init__(self, graph: SchemaGraph) -> None:
        self.graph = graph

    def _normalize(self, text: str) -> str:
        text = text.strip()
        text = re.sub(r"\s+", " ", text)
        return text

    def _extract_phrases(self, text: str) -> List[str]:
        tokens = _tokenize(text)
        phrases: Set[str] = set()
        for size in (1, 2, 3):
            for idx in range(len(tokens) - size + 1):
                phrases.add(" ".join(tokens[idx : idx + size]))
        capital_chunks = re.findall(r"([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)", text)
        phrases.update(capital_chunks)
        return sorted(phrases, key=len, reverse=True)

    def _mask_entities(self, text: str) -> str:
        return re.sub(r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)\b", "<ENT>", text)

    def _strategy_exact(self, tokens: Set[str], phrases: List[str]) -> Set[str]:
        hits: Set[str] = set()
        canon_tokens = {_canonical(t) for t in tokens}
        for label in self.graph.nodes:
            if _canonical(label) in canon_tokens:
                hits.add(label)
        for prop in {p for n in self.graph.nodes.values() for p in n.properties}:
            if _canonical(prop) in canon_tokens:
                hits.add(prop)
        for rel in {e.rel for e in self.graph.edges}:
            if _canonical(rel) in canon_tokens:
                hits.add(rel)
        for phrase in phrases:
            canon = _canonical(phrase)
            for label in self.graph.nodes:
                if canon == _canonical(label):
                    hits.add(label)
        return hits

    def _strategy_ner_mask(self, text: str) -> Set[str]:
        masked = self._mask_entities(text)
        tokens = {_canonical(t) for t in _tokenize(masked)}
        hits: Set[str] = set()
        for label in self.graph.nodes:
            if _canonical(label) in tokens:
                hits.add(label)
        for rel in {e.rel for e in self.graph.edges}:
            if _canonical(rel) in tokens:
                hits.add(rel)
        for prop in {p for n in self.graph.nodes.values() for p in n.properties}:
            if _canonical(prop) in tokens:
                hits.add(prop)
        return hits

    def _strategy_semantic(self, phrases: List[str], threshold: float = 0.74) -> Set[str]:
        hits: Set[str] = set()
        all_terms = list(self.graph.nodes.keys()) + list({e.rel for e in self.graph.edges}) + list(
            {p for n in self.graph.nodes.values() for p in n.properties}
        )
        for phrase in phrases:
            best = self.graph.semantic_matches(phrase, all_terms, limit=2)
            for name, score in best:
                if score >= threshold:
                    hits.add(name)
        return hits

    def _filtered_schema(self, hits: Set[str]) -> Tuple[Dict[str, SchemaNode], List[SchemaEdge]]:
        node_hits = {h for h in hits if h in self.graph.nodes}
        prop_hits = {h for h in hits if any(h in n.properties for n in self.graph.nodes.values())}
        rel_hits = {h for h in hits if any(h == e.rel for e in self.graph.edges)}

        candidate_nodes: Dict[str, SchemaNode] = {}
        if node_hits:
            for n in node_hits:
                candidate_nodes[n] = self.graph.nodes[n]
        else:
            candidate_nodes = dict(self.graph.nodes)

        edges: List[SchemaEdge] = []
        for edge in self.graph.edges:
            if edge.src in candidate_nodes or edge.dst in candidate_nodes or edge.rel in rel_hits:
                edges.append(edge)
        if not edges:
            edges = list(self.graph.edges)

        # Add nodes referenced by selected edges so adjacency stays consistent.
        for edge in edges:
            if edge.src in self.graph.nodes:
                candidate_nodes.setdefault(edge.src, self.graph.nodes[edge.src])
            if edge.dst in self.graph.nodes:
                candidate_nodes.setdefault(edge.dst, self.graph.nodes[edge.dst])

        # Pull in nodes that own matched properties.
        for node in self.graph.nodes.values():
            if any(prop in prop_hits for prop in node.properties):
                candidate_nodes.setdefault(node.name, node)

        return candidate_nodes, edges

    def _path_hints(self, nodes: Dict[str, SchemaNode]) -> List[str]:
        names = set(nodes.keys())
        paths = self.graph.shortest_paths(names, names, max_depth=3)
        hints: List[str] = []
        for path in paths:
            hints.append(" -> ".join([f"{edge.src}-[:{edge.rel}]->{edge.dst}" for edge in path]))
        return sorted(set(hints))

    def run(self, nl: str, feedback: List[str]) -> PreprocessResult:
        normalized = self._normalize(nl)
        phrases = self._extract_phrases(nl)
        tokens = set(_tokenize(normalized))

        exact_hits = self._strategy_exact(tokens, phrases)
        ner_hits = self._strategy_ner_mask(nl)
        semantic_hits = self._strategy_semantic(phrases)
        all_hits = exact_hits | ner_hits | semantic_hits

        nodes, edges = self._filtered_schema(all_hits)
        paths = self._path_hints(nodes)
        rel_hints = [e.descriptor() for e in edges]

        strategy_hits = {
            "exact": sorted(exact_hits),
            "ner_masked": sorted(ner_hits),
            "semantic": sorted(semantic_hits),
        }

        filtered = FilteredSchema(nodes=nodes, edges=edges, strategy_hits=strategy_hits, path_hints=paths)
        hints = paths + rel_hints + strategy_hits["exact"] + strategy_hits["semantic"]
        if feedback:
            hints += feedback[-2:]
        return PreprocessResult(
            raw_nl=nl,
            normalized_nl=normalized,
            phrases=phrases,
            filtered_schema=filtered,
            structural_hints=sorted(set(hints)),
        )


# -----------------------------------------------------------------------------
# Intent + linking orchestrator
# -----------------------------------------------------------------------------


def _links_to_hints(links: Dict[str, Any]) -> List[str]:
    hints: List[str] = []
    for nl in links.get("node_links") or []:
        alias, label = nl.get("alias"), nl.get("label")
        if alias and label:
            hints.append(f"{alias}:{label}")
    for rl in links.get("rel_links") or []:
        left, rel, right = rl.get("left_alias"), rl.get("rel"), rl.get("right_alias")
        if left and rel and right:
            hints.append(f"{left}-[:{rel}]->{right}")
    return hints


class IntentLinker:
    """Bridge between schema-agnostic preprocessing and the IR refiner using intent + linking."""

    def __init__(self, graph: SchemaGraph, model: str = DEFAULT_OPENAI_MODEL_GEN) -> None:
        self.graph = graph
        self.model = model

    def run(self, nl: str, pre: PreprocessResult, failures: List[str]) -> IntentLinkGuidance:
        frame, _ = draft_intent_frame(nl, pre.filtered_schema.describe(), self.model, failures)
        hits = (
            pre.filtered_schema.strategy_hits.get("exact", [])
            + pre.filtered_schema.strategy_hits.get("semantic", [])
            + pre.filtered_schema.strategy_hits.get("ner_masked", [])
        )
        links_raw, _ = link_schema(frame, nl, pre.filtered_schema.describe(), self.model, failures, heuristic_hits=hits)
        grounded = ground_links_to_schema(links_raw, self.graph)
        return IntentLinkGuidance(frame=frame, links=grounded)


# -----------------------------------------------------------------------------
# Validation stack
# -----------------------------------------------------------------------------


@dataclass
class SyntaxResult:
    ok: bool
    error: Optional[str] = None
    rows: Optional[Any] = None


class GraphLiteRunner:
    def __init__(
        self,
        *,
        db_path: Optional[str] = None,
        user: str = DEFAULT_DB_USER,
        schema: str = DEFAULT_DB_SCHEMA,
        graph: str = DEFAULT_DB_GRAPH,
    ) -> None:
        self._owns_db = db_path is None
        self._db_path = Path(db_path) if db_path else Path(tempfile.mkdtemp(prefix="graphlite_nl2gql_"))
        self._user = user
        self._schema = schema
        self._graph = graph
        self._db: Optional[GraphLite] = None
        self._session: Optional[str] = None
        self._ready = False

    def __enter__(self) -> "GraphLiteRunner":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def _ensure_ready(self) -> None:
        if self._ready:
            return
        self._db = GraphLite(str(self._db_path))
        self._session = self._db.create_session(self._user)
        bootstrap = [
            f"CREATE SCHEMA IF NOT EXISTS {self._schema}",
            f"SESSION SET SCHEMA {self._schema}",
            f"CREATE GRAPH IF NOT EXISTS {self._graph}",
            f"SESSION SET GRAPH {self._graph}",
        ]
        for stmt in bootstrap:
            self._db.execute(self._session, stmt)
        self._ready = True

    def close(self) -> None:
        if self._db:
            try:
                if self._session:
                    try:
                        self._db.close_session(self._session)
                    except GraphLiteError:
                        pass
                self._db.close()
            finally:
                self._db = None
                self._session = None

        if self._owns_db and self._db_path.exists():
            try:
                for path in sorted(self._db_path.rglob("*"), reverse=True):
                    if path.is_file():
                        path.unlink(missing_ok=True)
                    else:
                        path.rmdir()
                self._db_path.rmdir()
            except Exception:
                pass

    def validate(self, query: str) -> SyntaxResult:
        if not query.strip():
            return SyntaxResult(ok=False, error="empty query")
        try:
            self._ensure_ready()
            assert self._db is not None and self._session is not None
            rows = self._db.query(self._session, query.strip())
            return SyntaxResult(ok=True, rows=rows)
        except GraphLiteError as exc:
            return SyntaxResult(ok=False, error=exc.message)
        except Exception as exc:  # pragma: no cover
            return SyntaxResult(ok=False, error=str(exc))


# -----------------------------------------------------------------------------
# Prompt templates (intent + linking)
# -----------------------------------------------------------------------------


SYSTEM_INTENT = """You are a careful GraphQL/ISO GQL planner.
- Think in stages: understand intent, align to schema, plan graph traversal.
- Output only JSON with fields: targets, filters, metrics, order_by, limit, reasoning, path_hints.
- Use only labels/properties/relationships that exist in the schema_graph text.
- Preserve aggregates and grouping needs explicitly."""

USER_INTENT_TEMPLATE = """schema_graph:
{graph}

request: {nl}

Emit JSON:
{{
  "targets": ["entity or attribute names to return"],
  "filters": ["plain language constraints to enforce"],
  "metrics": ["aggregates or counts needed"],
  "order_by": ["sort instructions with directions"],
  "limit": "<integer or null>",
  "reasoning": "1-2 sentences of how to satisfy the request",
  "path_hints": ["likely traversals e.g., LabelA -REL_TYPE-> LabelB"]
}}"""


SYSTEM_LINK = """You are a schema linker like RAT-SQL/ResdSQL.
- Map natural-language mentions to concrete schema nodes/properties/relationships.
- Use only labels/properties/relationships that exist in the schema_graph (verbatim).
- Map plurals/synonyms to the closest schema label instead of repeating the NL wording (e.g., employees → Person if Person is the schema label).
- Prefer shortest valid paths; avoid inventing schema elements and avoid properties not present in the schema_graph.
- Output JSON with node_links, property_links, rel_links, and canonical aliases."""

USER_LINK_TEMPLATE = """schema_graph:
{graph}

intent_frame:
{frame}

heuristic_hits:
{hits}

Emit JSON:
{{
  "node_links": [{{"alias": "n1", "label": "<SchemaLabel>", "reason": "maps to an NL mention"}}],
  "property_links": [{{"alias": "n1", "property": "<property>", "reason": "attribute explicitly referenced"}}],
  "rel_links": [{{"left_alias": "n1", "rel": "<REL_TYPE>", "right_alias": "n2", "reason": "connects the mentioned entities"}}]
}}"""


# -----------------------------------------------------------------------------
# Intent + schema linking helpers
# -----------------------------------------------------------------------------


def draft_intent_frame(
    nl: str, schema_text: str, model: str, feedback: List[str]
) -> Tuple[Dict[str, Any], Optional[Dict[str, Any]]]:
    user = USER_INTENT_TEMPLATE.format(graph=schema_text, nl=nl)
    if feedback:
        user += "\n\nprevious_failures:\n- " + "\n- ".join(feedback[-5:])

    text, usage = chat_complete(model, SYSTEM_INTENT, user, temperature=0.2, top_p=0.9)
    frame = _safe_json_loads(text) or {}
    return frame, usage


def link_schema(
    frame: Dict[str, Any],
    nl: str,
    schema_text: str,
    model: str,
    feedback: List[str],
    heuristic_hits: Optional[Sequence[str]] = None,
) -> Tuple[Dict[str, Any], Optional[Dict[str, Any]]]:
    hits = heuristic_hits or []
    user = USER_LINK_TEMPLATE.format(
        graph=schema_text,
        frame=json.dumps(frame, indent=2),
        hits="\n".join(hits) if hits else "none",
    )
    if feedback:
        user += "\n\navoid_errors:\n- " + "\n- ".join(feedback[-3:])

    text, usage = chat_complete(model, SYSTEM_LINK, user, temperature=0.2, top_p=0.9)
    links = _safe_json_loads(text) or {}
    return links, usage


def _closest_schema_label(
    raw_label: str, alias: str, property_links: List[Dict[str, Any]], graph: SchemaGraph
) -> Optional[str]:
    """Pick the best schema label for an alias using name similarity + property overlap."""

    canonical_label = _canonical(raw_label)
    props_for_alias = {
        _canonical(pl["property"])
        for pl in property_links
        if pl.get("alias") == alias and pl.get("property")
    }

    best_label: Optional[str] = None
    best_score = 0.0
    for schema_label, node in graph.nodes.items():
        score = _ratio(canonical_label, _canonical(schema_label))
        if props_for_alias:
            overlap = props_for_alias & {_canonical(p) for p in node.properties}
            if overlap:
                score += 0.8 + 0.3 * len(overlap)
        if score > best_score:
            best_score = score
            best_label = schema_label

    return best_label if best_score >= 0.55 else None


def _closest_property(label: str, prop: str, graph: SchemaGraph) -> Optional[str]:
    """Map a non-existent property to the nearest valid one on the label."""

    if not graph.has_node(label):
        return None

    canonical_prop = _canonical(prop)
    best_prop: Optional[str] = None
    best_score = 0.0
    for candidate in graph.nodes[label].properties:
        score = _ratio(canonical_prop, _canonical(candidate))
        if score > best_score:
            best_score = score
            best_prop = candidate
    return best_prop if best_score >= 0.75 else None


def _closest_relationship(left_label: str, raw_rel: str, right_label: str, graph: SchemaGraph) -> Optional[str]:
    """Choose the best relationship that actually exists between two labels."""

    candidates = [e for e in graph.edges if e.src == left_label and e.dst == right_label]
    if not candidates:
        return None

    canonical_rel = _canonical(raw_rel)
    best: Optional[str] = None
    best_score = 0.0
    for edge in candidates:
        score = _ratio(canonical_rel, _canonical(edge.rel))
        if score > best_score:
            best_score = score
            best = edge.rel
    return best if best_score >= 0.6 else None


def ground_links_to_schema(links: Dict[str, Any], graph: SchemaGraph) -> Dict[str, Any]:
    """Normalize linker output to the actual schema to avoid hallucinated labels/edges."""

    node_links = links.get("node_links") or []
    property_links = links.get("property_links") or []
    rel_links = links.get("rel_links") or []

    alias_to_label: Dict[str, str] = {}
    grounded_nodes: List[Dict[str, Any]] = []
    for nl in node_links:
        alias, label = nl.get("alias"), nl.get("label")
        if not alias or not label:
            continue
        if graph.has_node(label):
            alias_to_label[alias] = label
            grounded_nodes.append({"alias": alias, "label": label, "reason": nl.get("reason")})
            continue
        mapped = _closest_schema_label(label, alias, property_links, graph)
        if mapped:
            alias_to_label[alias] = mapped
            grounded_nodes.append({"alias": alias, "label": mapped, "reason": f"normalized from {label}"})

    grounded_props: List[Dict[str, Any]] = []
    for pl in property_links:
        alias, prop = pl.get("alias"), pl.get("property")
        if not alias or not prop or alias not in alias_to_label:
            continue
        label = alias_to_label[alias]
        if graph.has_property(label, prop):
            grounded_props.append(pl)
            continue
        alt = _closest_property(label, prop, graph)
        if alt:
            new_pl = dict(pl)
            new_pl["property"] = alt
            grounded_props.append(new_pl)

    grounded_rels: List[Dict[str, Any]] = []
    for rl in rel_links:
        left, rel_name, right = rl.get("left_alias"), rl.get("rel"), rl.get("right_alias")
        if not left or not right or left not in alias_to_label or right not in alias_to_label:
            continue
        left_label, right_label = alias_to_label[left], alias_to_label[right]
        if graph.edge_exists(left_label, rel_name, right_label):
            grounded_rels.append(rl)
            continue
        alt = _closest_relationship(left_label, rel_name or "", right_label, graph)
        if alt:
            new_rl = dict(rl)
            new_rl["rel"] = alt
            grounded_rels.append(new_rl)

    grounded = {
        "node_links": grounded_nodes,
        "property_links": grounded_props,
        "rel_links": grounded_rels,
        "canonical_aliases": links.get("canonical_aliases", {}),
    }
    return grounded


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
            left_label = nodes.get(edge.left_alias, IRNode(edge.left_alias)).label
            right_label = nodes.get(edge.right_alias, IRNode(edge.right_alias)).label
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
            f"STRUCTURAL HINTS:\n{hint_text}\n\n"
            "Does the query satisfy the request?"
        )
        temps = [0.0, 0.2, 0.4]
        valid_votes = 0
        reasons: List[str] = []
        for temp in temps:
            verdict, _ = chat_complete(self.model, self.SYSTEM, user, temperature=temp, top_p=0.9, max_tokens=200)
            verdict_upper = verdict.strip().upper()
            if verdict_upper.startswith("VALID"):
                valid_votes += 1
            elif verdict_upper.startswith("INVALID:"):
                reasons.append(verdict.strip()[len("INVALID:") :].strip() or "unspecified reason")
            else:
                reasons.append(verdict.strip() or "logic validator unsure")
        if valid_votes > len(temps) // 2:
            return True, None
        reason = reasons[0] if reasons else "logic validator unsure"
        return False, reason


# -----------------------------------------------------------------------------
# Generator
# -----------------------------------------------------------------------------


@dataclass
class CandidateQuery:
    query: str
    reason: Optional[str] = None
    usage: Optional[Dict[str, Any]] = None


class QueryGenerator:
    SYSTEM = (
        "You are a cautious ISO GQL generator.\n"
        "- Use only schema labels/properties/relationships that appear in the provided filtered schema summary.\n"
        "- Do not invent names or traverse relationships that are not listed.\n"
        "- Use each relationship only between the labels it connects in the filtered schema summary; do not connect a relationship to any other label pair.\n"
        "- Build a single MATCH with aliases, clear WHERE filters, explicit RETURN, ORDER BY, and LIMIT when requested.\n"
        "- Use properties only on their owning label; when you need related attributes, traverse to that node instead of inventing fields on another label.\n"
        "- For comparisons across related nodes, create distinct aliases for each hop and compare their properties.\n"
        "- Prefer explicit traversals that follow the relationships given in the filtered schema summary rather than assuming shortcuts.\n"
        "- Use WITH only when necessary for aggregated filters, keeping the pipeline linear.\n"
        "- Keep output to ISO GQL; avoid subqueries, CALL, or schema modifications.\n"
        "- Follow path hints when they align with the request.\n"
        "- Emit strictly the JSON shape requested."
    )

    USER_TEMPLATE = """Normalized NL: {nl}

Filtered schema:
{schema_summary}

Intent frame:
{intent_frame}

Schema links (grounded):
{links}

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

    def generate(self, pre: PreprocessResult, failures: List[str], guidance: Optional[IntentLinkGuidance] = None) -> List[CandidateQuery]:
        failure_text = "- " + "\n- ".join(failures[-5:]) if failures else "none"
        intent_frame = json.dumps(guidance.frame, indent=2) if guidance else "none"
        links_text = json.dumps(guidance.links, indent=2) if guidance else "none"
        combined_hints = pre.structural_hints + (_links_to_hints(guidance.links) if guidance else [])
        user = self.USER_TEMPLATE.format(
            nl=pre.normalized_nl,
            schema_summary=pre.filtered_schema.summary_lines(),
            intent_frame=intent_frame,
            links=links_text,
            hints="\n".join(sorted(set(combined_hints))) if combined_hints else "none",
            failures=failure_text,
        )
        raw, usage = chat_complete(self.model, self.SYSTEM, user, temperature=0.15, top_p=0.9, max_tokens=700)
        data = _safe_json_loads(raw) or {}
        candidates: List[CandidateQuery] = []
        for entry in data.get("queries") or []:
            query = (entry.get("query") or "").strip()
            if query:
                candidates.append(CandidateQuery(query=query, reason=entry.get("reason"), usage=usage))
        if not candidates and raw.strip():
            candidates.append(CandidateQuery(query=_clean_block(raw), usage=usage))
        return candidates


# -----------------------------------------------------------------------------
# Refiner
# -----------------------------------------------------------------------------


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
        self.runner = runner or GraphLiteRunner(db_path=db_path or DEFAULT_DB_PATH)
        self.max_loops = max_loops

    def _repair_ir_schema(self, ir: ISOQueryIR) -> bool:
        changed = False
        for edge in ir.edges:
            # Ensure nodes exist in the IR
            left_node = ir.nodes.setdefault(edge.left_alias, IRNode(alias=edge.left_alias))
            right_node = ir.nodes.setdefault(edge.right_alias, IRNode(alias=edge.right_alias))
            left_label = left_node.label
            right_label = right_node.label

            # If labels already align with a schema edge, nothing to repair.
            if left_label and right_label and any(
                e.src == left_label and e.rel == edge.rel and e.dst == right_label for e in self.graph.edges
            ):
                continue

            # Try to fill missing labels based on any schema edge with the same relationship.
            for schema_edge in self.graph.edges:
                if schema_edge.rel != edge.rel:
                    continue

                # We only mutate missing labels to avoid collapsing distinct aliases that share a label.
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
        return changed

    def _heuristic_logic_accept(self, ir: ISOQueryIR, hints: List[str]) -> bool:
        """
        Generic structural fallback: if schema/syntax are clean and the query
        already covers most structural hints (links/path hints), accept even if
        the LLM logic vote is unsure. This avoids schema-specific patterns.
        """

        if not hints:
            return False

        edge_hints = {h for h in hints if "-[:".lower() in h.lower()}
        label_hints = {h for h in hints if ":" in h and "-[:".lower() not in h.lower()}

        ir_edge_tokens = {f"{e.left_alias.lower()}-[:{e.rel.lower()}]->{e.right_alias.lower()}" for e in ir.edges}
        ir_label_tokens = {f"{alias.lower()}:{node.label.lower()}" for alias, node in ir.nodes.items() if node.label}

        def _match_ratio(hint_set: Set[str], token_set: Set[str]) -> float:
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
    ) -> ValidationBundle:
        ir, parse_errors = ISOQueryIR.parse(candidate.query)
        repaired = False
        schema_errors: List[str] = []
        rendered = candidate.query
        if ir:
            repaired = self._repair_ir_schema(ir)
            schema_errors = schema_validator.validate(ir)
            rendered = ir.render()
        syntax = self.runner.validate(rendered)
        logic_valid = False
        logic_reason: Optional[str] = None
        if ir:
            logic_valid, logic_reason = self.logic_validator.validate(
                nl, pre.filtered_schema.summary_lines(), rendered, hints
            )
            if not logic_valid and self._heuristic_logic_accept(ir, hints):
                logic_valid = True
                logic_reason = None
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
        intent_linker: IntentLinker,
        spinner: Optional[Spinner],
    ) -> Tuple[str, List[Dict[str, Any]]]:
        failures: List[str] = []
        timeline: List[Dict[str, Any]] = []
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
                combined_hints = sorted(
                    set(pre.structural_hints + _links_to_hints(guidance.links) + (frame_hints or []))
                )
                if spinner:
                    spinner.update(f"[attempt {attempt}] generating candidates...")
                candidates = self.generator.generate(pre, failures, guidance)
                timeline.append({"attempt": attempt, "phase": "generate", "candidates": [c.query for c in candidates]})
                if not candidates:
                    failures.append("generator returned no candidates")
                    continue

                for candidate in candidates:
                    bundle = self._evaluate_candidate(nl, pre, candidate, schema_validator, combined_hints)
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
                        and (bundle.syntax_result.ok or bundle.logic_valid)
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


# -----------------------------------------------------------------------------
# Pipeline orchestration
# -----------------------------------------------------------------------------


class NL2GQLPipeline:
    def __init__(
        self,
        schema_context: str,
        *,
        gen_model: str = DEFAULT_OPENAI_MODEL_GEN,
        fix_model: str = DEFAULT_OPENAI_MODEL_FIX,
        db_path: Optional[str] = DEFAULT_DB_PATH,
        max_refinements: int = 3,
    ) -> None:
        self.schema_graph = SchemaGraph.from_text(schema_context)
        if not self.schema_graph.nodes:
            raise RuntimeError("schema parsing produced no nodes")
        self.preprocessor = Preprocessor(self.schema_graph)
        self.intent_linker = IntentLinker(self.schema_graph, model=gen_model)
        self.generator = QueryGenerator(model=gen_model)
        self.refiner = Refiner(
            self.schema_graph,
            self.generator,
            logic_validator=LogicValidator(model=fix_model),
            db_path=db_path,
            max_loops=max_refinements,
        )

    def run(self, nl: str, *, spinner: Optional[Spinner] = None) -> Tuple[str, List[Dict[str, Any]]]:
        return self.refiner.run(nl, self.preprocessor, self.intent_linker, spinner)


# -----------------------------------------------------------------------------
# Sample-suite runner
# -----------------------------------------------------------------------------


def _extract_queries_from_file(path: str) -> List[str]:
    queries: List[str] = []
    pattern = re.compile(r'--nl\s+"([^"]+)"')
    with open(path, "r", encoding="utf-8") as fh:
        for line in fh:
            match = pattern.search(line)
            if match:
                queries.append(match.group(1))
    return queries


def run_sample_suite(
    schema_path: str,
    suite_path: str,
    *,
    max_iterations: int = 3,
    verbose: bool = False,
    db_path: Optional[str] = None,
) -> List[Dict[str, Any]]:
    if Path(schema_path).exists():
        schema_text = Path(schema_path).read_text(encoding="utf-8")
    else:
        schema_text = schema_path
    nls = _extract_queries_from_file(suite_path)
    results: List[Dict[str, Any]] = []
    for idx, nl in enumerate(nls, 1):
        spinner = Spinner(enabled=verbose)
        spinner.start(f"[suite {idx}] running")
        _reset_usage_log()
        pipeline = NL2GQLPipeline(schema_text, max_refinements=max_iterations, db_path=db_path)
        try:
            query, timeline = pipeline.run(nl, spinner=spinner)
            spinner.stop(f"[suite {idx}] ok", color="green")
            usage = _usage_totals()
            results.append({"nl": nl, "query": query, "timeline": timeline, "usage": usage, "success": True})
        except PipelineFailure as exc:
            spinner.stop(f"[suite {idx}] failed", color="red")
            usage = _usage_totals()
            results.append(
                {
                    "nl": nl,
                    "error": str(exc),
                    "timeline": exc.timeline,
                    "failures": exc.failures,
                    "usage": usage,
                    "success": False,
                }
            )
        except Exception as exc:
            spinner.stop(f"[suite {idx}] failed", color="red")
            usage = _usage_totals()
            results.append({"nl": nl, "error": str(exc), "usage": usage, "success": False})
    return results


# -----------------------------------------------------------------------------
# CLI
# -----------------------------------------------------------------------------


def _fmt_block(text: str, indent: int = 6) -> str:
    pad = " " * indent
    return "\n".join(pad + line for line in text.splitlines())


def print_timeline(nl_query: str, validation_log: List[Dict[str, Any]], max_attempts: int) -> None:
    print("\n" + "=" * 80)
    print("PIPELINE EXECUTION SUMMARY")
    print("=" * 80)
    print(f"Query: {nl_query}")
    print(f"Max Attempts: {max_attempts}")

    grouped: Dict[int, List[Dict[str, Any]]] = defaultdict(list)
    for entry in validation_log:
        grouped[entry.get("attempt", 0)].append(entry)

    print("\nTimeline (per attempt):")
    for attempt in sorted(grouped):
        print("-" * 80)
        print(f"Attempt {attempt}")
        for entry in grouped[attempt]:
            phase = entry.get("phase")
            if phase == "intent":
                print("  • Intent frame")
                print(_fmt_block(json.dumps(entry.get("frame"), indent=2)))
            elif phase == "link":
                print("  • Schema links")
                print(_fmt_block(json.dumps(entry.get("links"), indent=2)))
            elif phase == "generate":
                print("  • Candidates")
                print(_fmt_block(json.dumps(entry.get("candidates"), indent=2)))
            else:
                print("  • Candidate evaluation")
                details = {k: v for k, v in entry.items() if k not in {"attempt"}}
                print(_fmt_block(json.dumps(details, indent=2)))
    print("=" * 80)


def read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read().strip()


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Generate ISO GQL queries from natural language (schema-agnostic).")
    parser.add_argument("--nl", help="Natural language request")
    parser.add_argument("--schema-file", help="Path to schema context text")
    parser.add_argument("--schema", help="Schema context as a string (overrides --schema-file)")
    parser.add_argument("--max-attempts", type=int, default=3, help="Max refinement loops")
    parser.add_argument("--gen-model", help="OpenAI model for generation (default: gpt-4o-mini)")
    parser.add_argument("--fix-model", help="OpenAI model for fixes/logic validation (default: gpt-4o-mini)")
    parser.add_argument("--verbose", action="store_true", help="Print attempt timeline")
    parser.add_argument("--sample-suite", action="store_true", help="Run all queries in sample_queries.txt")
    parser.add_argument("--suite-file", default="nl2gql/sample_queries.txt", help="Path to sample queries list")
    parser.add_argument("--db-path", help="GraphLite DB path for syntax validation (defaults to temp or NL2GQL_DB_PATH)")
    parser.add_argument("--spinner", dest="spinner", action="store_true", help="Show live spinner updates when running single queries")
    parser.add_argument("--no-spinner", dest="spinner", action="store_false")
    parser.set_defaults(spinner=None)

    args = parser.parse_args(argv)

    schema_context: Optional[str] = None
    if args.schema is not None:
        potential_path = Path(args.schema)
        if potential_path.exists():
            schema_context = read_text(str(potential_path))
        else:
            schema_context = args.schema
    elif args.schema_file:
        schema_context = read_text(args.schema_file)

    if args.sample_suite:
        if not args.schema_file and not args.schema:
            print("error: schema context required for sample suite", file=sys.stderr)
            return 1
        schema_path = args.schema_file or args.schema
        assert schema_path
        results = run_sample_suite(
            schema_path,
            args.suite_file,
            max_iterations=args.max_attempts,
            verbose=args.verbose,
            db_path=args.db_path or DEFAULT_DB_PATH,
        )
        success = all(r.get("success") for r in results)
        for res in results:
            if res.get("success"):
                print(f"[ok] {res['nl']}")
                print(_fmt_block(res["query"], indent=4))
                if args.verbose:
                    usage = res.get("usage", {})
                    print(_fmt_block(f"token usage → prompt: {usage.get('prompt_tokens', 0)}, completion: {usage.get('completion_tokens', 0)}, total: {usage.get('total_tokens', 0)}", indent=4))
            else:
                print(f"[fail] {res['nl']}: {res.get('error')}")
        return 0 if success else 2

    if not schema_context:
        print("error: schema context is required via --schema or --schema-file", file=sys.stderr)
        return 1
    if not args.nl:
        print("error: --nl is required when not running the sample suite", file=sys.stderr)
        return 1

    spinner = Spinner(enabled=args.spinner if args.spinner is not None else sys.stdout.isatty())
    _reset_usage_log()
    spinner.start("Starting pipeline...")
    try:
        pipeline = NL2GQLPipeline(
            schema_context,
            gen_model=args.gen_model or DEFAULT_OPENAI_MODEL_GEN,
            fix_model=args.fix_model or DEFAULT_OPENAI_MODEL_FIX,
            db_path=args.db_path or DEFAULT_DB_PATH,
            max_refinements=args.max_attempts,
        )
        query, timeline = pipeline.run(args.nl, spinner=spinner)
        spinner.stop("✓ Query generated.", color="green")
        if args.verbose:
            print_timeline(args.nl, timeline, args.max_attempts)
            usage = _usage_totals()
            print(f"\nToken usage → prompt: {usage['prompt_tokens']}, completion: {usage['completion_tokens']}, total: {usage['total_tokens']}")
        print(query)
        return 0
    except PipelineFailure as exc:
        spinner.stop("✗ Pipeline failed.", color="red")
        if args.verbose:
            print_timeline(args.nl, exc.timeline, args.max_attempts)
            if exc.failures:
                print("Failures:")
                for f in exc.failures:
                    print(f"  - {f}")
            usage = _usage_totals()
            print(f"\nToken usage → prompt: {usage['prompt_tokens']}, completion: {usage['completion_tokens']}, total: {usage['total_tokens']}")
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        return 1
    except Exception as exc:
        spinner.stop("✗ Pipeline failed.", color="red")
        if args.verbose:
            usage = _usage_totals()
            print(f"\nToken usage → prompt: {usage['prompt_tokens']}, completion: {usage['completion_tokens']}, total: {usage['total_tokens']}")
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
