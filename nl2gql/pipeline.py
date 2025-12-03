"""Schema-grounded NL → ISO GQL pipeline with RAT-SQL style structure.

We replace single-shot prompting with a multi-stage, graph-aware pipeline:
1) Parse schema into a graph (nodes, properties, relationships).
2) Draft an intent frame (targets, filters, metrics, ordering).
3) Link intent elements to concrete schema nodes/props/edges.
4) Plan a constrained AST over the schema graph.
5) Render to ISO GQL, then validate syntax (GraphLite) + logic (LLM jury).

The flow mirrors techniques used in RAT-SQL / ResdSQL / PICARD:
- Schema is encoded as an explicit graph.
- Intent and schema are encoded jointly for planning.
- Constrained decoding via an intermediate AST with schema checks.
- Multi-pass validation with self-repair.

Requirements (install into your venv):
    pip install openai tenacity python-dotenv
    pip install -e bindings/python   # from repo root

Environment (config.env in this folder is read automatically if present):
    OPENAI_API_KEY=sk-...
    OPENAI_MODEL_GEN=gpt-4o-mini     # constrained by request
    OPENAI_MODEL_FIX=gpt-4o-mini
    NL2GQL_DB_PATH=./.nl2gql_cache
    NL2GQL_USER=admin
    NL2GQL_SCHEMA=nl2gql
    NL2GQL_GRAPH=scratch

Usage (CLI):
    python nl2gql/pipeline.py --nl "find people older than 30" \
      --schema-file ./schema.txt --verbose
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import sys
import tempfile
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple

from tenacity import retry, stop_after_attempt, wait_fixed

try:  # Local config
    from dotenv import load_dotenv
except ImportError:  # pragma: no cover - optional helper
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


def _client() -> OpenAI:
    global _client_singleton
    if _client_singleton is None:
        _client_singleton = OpenAI()
    return _client_singleton


@retry(stop=stop_after_attempt(3), wait=wait_fixed(0.2))
def chat_complete(
    model: str,
    system: str,
    user: str,
    *,
    temperature: float = 0.2,
    top_p: float = 0.9,
    max_tokens: int = 600,
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
        return text, usage
    return text, None


def _clean_block(text: str) -> str:
    """Strip fences and surrounding whitespace."""

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


def _format_literal(value: Any) -> str:
    """Render Python values into ISO GQL literal syntax."""

    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        return "null"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(_format_literal(v) for v in value) + "]"

    # Default: treat as string
    text = str(value)
    if not (text.startswith("'") and text.endswith("'")):
        text = text.replace("\\", "\\\\").replace("'", "\\'")
        return f"'{text}'"
    return text


def _shuffle_schema_context(schema_context: str) -> str:
    """Shuffle schema lines to reduce positional bias during validation."""

    lines = schema_context.splitlines()
    if len(lines) < 2:
        return schema_context

    shuffled = lines[:]
    random.shuffle(shuffled)
    return "\n".join(shuffled)


# -----------------------------------------------------------------------------
# Schema graph parsing + helpers
# -----------------------------------------------------------------------------


def _canonical(name: str) -> str:
    return re.sub(r"[^a-z0-9]", "", name.lower())


@dataclass
class SchemaNode:
    name: str
    properties: List[str] = field(default_factory=list)

    def prompt_line(self) -> str:
        props = ", ".join(self.properties) if self.properties else "no properties listed"
        return f"{self.name}: {props}"


@dataclass
class SchemaEdge:
    src: str
    rel: str
    dst: str

    def prompt_line(self) -> str:
        return f"({self.src})-[:{self.rel}]->({self.dst})"


@dataclass
class SchemaGraph:
    nodes: Dict[str, SchemaNode]
    edges: List[SchemaEdge]

    @classmethod
    def from_text(cls, schema_context: str) -> "SchemaGraph":
        nodes: Dict[str, SchemaNode] = {}
        edges: List[SchemaEdge] = []

        for raw in schema_context.splitlines():
            line = raw.strip()
            if not line or line.startswith("#"):
                continue

            # Normalize bullet prefixes so patterns match.
            if line.startswith("- "):
                line = line[2:].strip()
            if line.startswith("* "):
                line = line[2:].strip()

            rel_match = re.match(r"^\(?([A-Za-z0-9_]+)\)?-?\s*\[:([A-Za-z0-9_]+)\]\s*->\s*\(?([A-Za-z0-9_]+)\)?", line)
            if rel_match:
                edges.append(SchemaEdge(rel_match.group(1), rel_match.group(2), rel_match.group(3)))
                continue

            # Entity with properties: "- Label: id, name, attr"
            ent_match = re.match(r"^-?\s*([A-Za-z0-9_]+)\s*:\s*(.+)$", line)
            if ent_match:
                name = ent_match.group(1).strip()
                props_text = ent_match.group(2)
                props = [p.strip() for p in re.split(r"[;,]", props_text) if p.strip()]
                node = nodes.get(name) or SchemaNode(name=name)
                node.properties = sorted(set(node.properties + props))
                nodes[name] = node
                continue

        return cls(nodes=nodes, edges=edges)

    def has_node(self, name: str) -> bool:
        return name in self.nodes

    def has_property(self, node: str, prop: str) -> bool:
        return node in self.nodes and prop in self.nodes[node].properties

    def edge_exists(self, left: str, rel: str, right: str) -> bool:
        return any(e.src == left and e.rel == rel and e.dst == right for e in self.edges)

    def describe(self) -> str:
        node_lines = [f"- {n.prompt_line()}" for n in self.nodes.values()]
        edge_lines = [f"- {e.prompt_line()}" for e in self.edges]
        return "ENTITIES:\n" + "\n".join(node_lines) + "\nRELATIONSHIPS:\n" + "\n".join(edge_lines)

    def heuristic_candidates(self, nl: str) -> List[str]:
        """Crude lexical hints for schema linking."""

        tokens = set(re.findall(r"[a-zA-Z][a-zA-Z0-9_]*", nl.lower()))
        hints: List[str] = []
        for node in self.nodes.values():
            hit_props = [p for p in node.properties if _canonical(p) in tokens or p.lower() in tokens]
            if hit_props:
                hints.append(f"{node.name}: {', '.join(hit_props)}")
        for edge in self.edges:
            if _canonical(edge.rel) in tokens:
                hints.append(f"edge {edge.prompt_line()}")
        return hints


# -----------------------------------------------------------------------------
# GraphLite syntax validator
# -----------------------------------------------------------------------------


class GraphLiteValidator:
    """Lightweight syntax validator backed by the GraphLite Python SDK."""

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

    def __enter__(self) -> "GraphLiteValidator":
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

    def validate(self, query: str) -> Tuple[bool, Optional[str]]:
        if not query.strip():
            return False, "empty query"

        try:
            self._ensure_ready()
            assert self._db is not None and self._session is not None
            self._db.query(self._session, query.strip())
            return True, None
        except GraphLiteError as exc:
            return False, exc.message
        except Exception as exc:  # pragma: no cover
            return False, str(exc)


# -----------------------------------------------------------------------------
# Prompt templates (multi-stage)
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
- Prefer shortest valid paths; avoid inventing schema elements.
- Use distinct aliases when the same label appears in multiple roles (e.g., home_city vs work_city); when both residence and workplace/location cities are mentioned, allocate separate city aliases.
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


SYSTEM_AST = """You are a constrained ISO GQL AST builder.
- Use only schema labels/properties/relationships provided.
- Build a single MATCH with all paths separated by commas.
- Push property predicates into WHERE (no aggregates in WHERE).
- Support alias-to-alias comparisons using filter values like {"ref_alias": "a2", "ref_property": "id"} for expressions such as a1.id <> a2.id.
- Filters must use plain properties (no dotted paths). If a filter references a related node’s property, place the filter on that node’s alias rather than encoding the relationship in the property name.
- Use COUNT/AVG/MIN/MAX/SUM in RETURN with aliases when aggregating.
- Output JSON AST with fields: nodes, relationships, filters, returns, order_by, limit, notes.
- Avoid subqueries and CALL; no inline MATCH/EXISTS; one MATCH only."""

USER_AST_TEMPLATE = """schema_graph:
{graph}

intent_frame:
{frame}

links:
{links}

Emit JSON:
{{
  "nodes": [{{"alias": "n1", "label": "<SchemaLabel>"}}],
  "relationships": [{{"left_alias": "n1", "rel": "<REL_TYPE>", "right_alias": "n2"}}],
  "filters": [{{"alias": "n1", "property": "<property>", "op": ">=", "value": 30}}, {{"alias": "n1", "property": "id", "op": "<>", "value": {{"ref_alias": "n2", "ref_property": "id"}}}}],
  "returns": [{{"expr": "n2.some_field", "alias": "result"}}, {{"expr": "COUNT(n1)", "alias": "total"}}],
  "order_by": [{{"expr": "total", "direction": "DESC"}}],
  "limit": 10,
  "notes": "concise explanation of grouping/aggregation choices"
}}"""


SYSTEM_VALIDATE_LOGIC = (
    "You judge if an ISO GQL query logically satisfies the natural language request using the provided schema. "
    "Be conservative: reply VALID only when all requested conditions, joins, and groupings are present. "
    "Reply INVALID with a short reason otherwise. Respond only with 'VALID' or 'INVALID: <reason>'."
)

USER_VALIDATE_LOGIC_TEMPLATE = (
    "SCHEMA:\n{schema_context}\n\n"
    "INTENT FRAME:\n{frame}\n\n"
    "REQUEST: {nl}\n\n"
    "GENERATED QUERY:\n{query}\n\n"
    "Does this query logically satisfy the request?"
)


# -----------------------------------------------------------------------------
# Stage 1: Intent framing
# -----------------------------------------------------------------------------


def draft_intent_frame(
    nl: str, graph: SchemaGraph, model: str, feedback: List[str]
) -> Tuple[Dict[str, Any], Optional[Dict[str, Any]]]:
    user = USER_INTENT_TEMPLATE.format(graph=graph.describe(), nl=nl)
    if feedback:
        user += "\n\nprevious_failures:\n- " + "\n- ".join(feedback[-5:])

    text, usage = chat_complete(model, SYSTEM_INTENT, user, temperature=0.2, top_p=0.9)
    frame = _safe_json_loads(text) or {}
    return frame, usage


# -----------------------------------------------------------------------------
# Stage 2: Schema linking
# -----------------------------------------------------------------------------


def link_schema(
    frame: Dict[str, Any],
    nl: str,
    graph: SchemaGraph,
    model: str,
    feedback: List[str],
) -> Tuple[Dict[str, Any], Optional[Dict[str, Any]]]:
    hits = graph.heuristic_candidates(nl)
    user = USER_LINK_TEMPLATE.format(
        graph=graph.describe(),
        frame=json.dumps(frame, indent=2),
        hits="\n".join(hits) if hits else "none",
    )
    if feedback:
        user += "\n\navoid_errors:\n- " + "\n- ".join(feedback[-3:])

    text, usage = chat_complete(model, SYSTEM_LINK, user, temperature=0.2, top_p=0.9)
    links = _safe_json_loads(text) or {}
    return links, usage


# -----------------------------------------------------------------------------
# Stage 3: AST planning
# -----------------------------------------------------------------------------


def plan_ast(
    frame: Dict[str, Any],
    links: Dict[str, Any],
    graph: SchemaGraph,
    model: str,
    feedback: List[str],
) -> Tuple[Dict[str, Any], Optional[Dict[str, Any]]]:
    user = USER_AST_TEMPLATE.format(
        graph=graph.describe(),
        frame=json.dumps(frame, indent=2),
        links=json.dumps(links, indent=2),
    )
    if feedback:
        user += "\n\nfix_these:\n- " + "\n- ".join(feedback[-4:])

    text, usage = chat_complete(model, SYSTEM_AST, user, temperature=0.25, top_p=0.9, max_tokens=700)
    ast = _safe_json_loads(text) or {}
    return ast, usage


# -----------------------------------------------------------------------------
# AST validation + rendering
# -----------------------------------------------------------------------------


def validate_ast(ast: Dict[str, Any], graph: SchemaGraph) -> List[str]:
    errors: List[str] = []
    nodes = ast.get("nodes") or []
    relationships = ast.get("relationships") or []
    filters = ast.get("filters") or []
    returns = ast.get("returns") or []

    node_labels = {n.get("alias"): n.get("label") for n in nodes if n.get("alias") and n.get("label")}

    for alias, label in node_labels.items():
        if not graph.has_node(label):
            errors.append(f"Unknown label for alias {alias}: {label}")

    for rel in relationships:
        left, rel_name, right = rel.get("left_alias"), rel.get("rel"), rel.get("right_alias")
        if left not in node_labels or right not in node_labels:
            errors.append(f"Relationship references unknown alias: {rel}")
            continue
        if not graph.edge_exists(node_labels[left], rel_name, node_labels[right]):
            errors.append(f"Edge not in schema: {node_labels[left]}-[:{rel_name}]->{node_labels[right]}")

    for flt in filters:
        alias, prop = flt.get("alias"), flt.get("property")
        if alias not in node_labels or not prop:
            errors.append(f"Filter references unknown alias/property: {flt}")
            continue
        if not graph.has_property(node_labels[alias], prop):
            errors.append(f"Unknown property {prop} on {node_labels[alias]}")
        value = flt.get("value")
        if isinstance(value, dict) and "ref_alias" in value and "ref_property" in value:
            ref_alias, ref_prop = value.get("ref_alias"), value.get("ref_property")
            if ref_alias not in node_labels:
                errors.append(f"Filter reference alias not defined: {ref_alias}")
            elif not graph.has_property(node_labels[ref_alias], ref_prop):
                errors.append(f"Filter references unknown property {ref_prop} on {node_labels[ref_alias]}")

    # Returns sanity: ensure expressions reference known aliases/props
    for ret in returns:
        expr = ret.get("expr", "")
        for match in re.findall(r"([a-zA-Z_][a-zA-Z0-9_]*)\.([a-zA-Z_][a-zA-Z0-9_]*)", expr):
            alias, prop = match
            if alias not in node_labels:
                errors.append(f"Return references unknown alias {alias} in {expr}")
            elif not graph.has_property(node_labels[alias], prop):
                errors.append(f"Return references unknown property {prop} on {node_labels[alias]}")

    return errors


def _pattern_for_relationship(left_alias: str, left_label: str, rel: str, right_alias: str, right_label: str) -> str:
    return f"({left_alias}:{left_label})-[:{rel}]->({right_alias}:{right_label})"


def _rewrite_filter_relationship_props(ast: Dict[str, Any]) -> bool:
    """
    Normalize filters where the property erroneously encodes a relationship (e.g., \"LIVES_IN.name\").
    If a filter references alias A with property \"REL.prop\" and there is a relationship
    A-[:REL]->B, rewrite the filter to alias=B, property=prop.
    """

    filters = ast.get("filters") or []
    relationships = ast.get("relationships") or []

    rel_map: Dict[str, Dict[str, List[str]]] = {}
    for rel in relationships:
        left, rel_name, right = rel.get("left_alias"), rel.get("rel"), rel.get("right_alias")
        if left and rel_name and right:
            rel_map.setdefault(left, {}).setdefault(rel_name, []).append(right)

    changed = False
    for flt in filters:
        alias = flt.get("alias")
        prop = flt.get("property")
        if not alias or not isinstance(prop, str) or "." not in prop:
            continue

        prefix, _, tail = prop.partition(".")
        targets = rel_map.get(alias, {}).get(prefix)
        if targets and tail:
            flt["alias"] = targets[0]
            flt["property"] = tail
            changed = True

    return changed


def render_ast_to_gql(ast: Dict[str, Any], graph: SchemaGraph) -> str:
    nodes = ast.get("nodes") or []
    relationships = ast.get("relationships") or []
    filters = ast.get("filters") or []
    returns = ast.get("returns") or []
    order_by = ast.get("order_by") or []
    limit = ast.get("limit")

    node_labels = {n["alias"]: n["label"] for n in nodes if "alias" in n and "label" in n}

    patterns: List[str] = []
    for rel in relationships:
        left_alias, rel_name, right_alias = rel.get("left_alias"), rel.get("rel"), rel.get("right_alias")
        if left_alias in node_labels and right_alias in node_labels and rel_name:
            patterns.append(
                _pattern_for_relationship(left_alias, node_labels[left_alias], rel_name, right_alias, node_labels[right_alias])
            )

    # Include isolated nodes that were not part of relationships
    connected_aliases = {alias for rel in relationships for alias in (rel.get("left_alias"), rel.get("right_alias"))}
    for alias, label in node_labels.items():
        if alias not in connected_aliases:
            patterns.append(f"({alias}:{label})")

    match_clause = "MATCH " + ", ".join(patterns)

    def _render_operand(value: Any) -> str:
        if isinstance(value, dict) and "ref_alias" in value and "ref_property" in value:
            return f"{value['ref_alias']}.{value['ref_property']}"
        return _format_literal(value)

    where_parts: List[str] = []
    for flt in filters:
        alias, prop, op, value = flt.get("alias"), flt.get("property"), flt.get("op"), flt.get("value")
        if alias and prop and op:
            where_parts.append(f"{alias}.{prop} {op} {_render_operand(value)}")
    where_clause = ""
    if where_parts:
        where_clause = "WHERE " + " AND ".join(where_parts)

    return_parts: List[str] = []
    for ret in returns:
        expr = ret.get("expr")
        alias = ret.get("alias")
        if expr:
            if alias:
                return_parts.append(f"{expr} AS {alias}")
            else:
                return_parts.append(expr)
    return_clause = "RETURN " + ", ".join(return_parts)

    order_clause = ""
    if order_by:
        items = []
        for ob in order_by:
            expr = ob.get("expr")
            direction = ob.get("direction", "ASC").upper()
            if expr:
                items.append(f"{expr} {direction}")
        if items:
            order_clause = "ORDER BY " + ", ".join(items)

    limit_clause = f"LIMIT {int(limit)}" if isinstance(limit, int) and limit > 0 else ""

    parts = [match_clause]
    if where_clause:
        parts.append(where_clause)
    parts.append(return_clause)
    if order_clause:
        parts.append(order_clause)
    if limit_clause:
        parts.append(limit_clause)

    return "\n".join(parts)


# -----------------------------------------------------------------------------
# Stage 4: Logic validation (LLM committee)
# -----------------------------------------------------------------------------


def validate_logical_correctness(
    nl: str,
    schema_context: str,
    query: str,
    frame: Dict[str, Any],
    model: str = DEFAULT_OPENAI_MODEL_FIX,
) -> Tuple[bool, Optional[str], Optional[Dict[str, Any]]]:
    temperatures = [0.0, 0.2, 0.4, 0.0, 0.2]
    votes = 0
    total = len(temperatures)
    reasons: List[str] = []
    samples: List[Dict[str, Any]] = []
    usage_totals = {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}
    any_usage = False

    for temp in temperatures:
        user = USER_VALIDATE_LOGIC_TEMPLATE.format(
            schema_context=_shuffle_schema_context(schema_context),
            frame=json.dumps(frame, indent=2),
            nl=nl,
            query=query,
        )
        result, usage = chat_complete(model, SYSTEM_VALIDATE_LOGIC, user, temperature=temp, top_p=0.9, max_tokens=150)
        verdict_raw = result.strip()
        verdict_upper = verdict_raw.upper()

        is_valid = False
        reason: Optional[str] = None
        if verdict_upper.startswith("VALID"):
            is_valid = True
            votes += 1
        elif verdict_upper.startswith("INVALID:"):
            reason = verdict_raw[len("INVALID:") :].strip() or "unspecified reason"
            reasons.append(reason)
        else:
            reason = f"Unexpected validation response: {verdict_raw}"
            reasons.append(reason)

        sample_entry: Dict[str, Any] = {"temperature": temp, "verdict": "VALID" if is_valid else "INVALID"}
        if reason:
            sample_entry["reason"] = reason
        samples.append(sample_entry)

        if usage:
            any_usage = True
            usage_totals["prompt_tokens"] += usage.get("prompt_tokens", 0)
            usage_totals["completion_tokens"] += usage.get("completion_tokens", 0)
            usage_totals["total_tokens"] += usage.get("total_tokens", 0)

    usage_summary: Optional[Dict[str, Any]] = None
    if any_usage:
        usage_summary = {**usage_totals, "samples": samples, "valid_votes": votes, "total_votes": total}

    if votes > total // 2:
        return True, None, usage_summary

    reason_summary = reasons[0] if reasons else "no consensus"
    if len(reasons) > 1:
        most_common = Counter(reasons).most_common(1)[0][0]
        reason_summary = most_common
    return False, reason_summary, usage_summary


# -----------------------------------------------------------------------------
# Generation pipeline orchestrator
# -----------------------------------------------------------------------------


def generate_isogql(
    nl: str,
    schema_context: str,
    *,
    max_attempts: int = 3,
    gen_model: Optional[str] = None,
    fix_model: Optional[str] = None,
    validator: Optional[GraphLiteValidator] = None,
) -> Tuple[Optional[str], List[Dict[str, Any]], List[Dict[str, Any]]]:
    return generate_isogql_with_progress(
        nl,
        schema_context,
        max_attempts=max_attempts,
        gen_model=gen_model,
        fix_model=fix_model,
        validator=validator,
        progress=None,
    )


def generate_isogql_with_progress(
    nl: str,
    schema_context: str,
    *,
    max_attempts: int = 3,
    gen_model: Optional[str] = None,
    fix_model: Optional[str] = None,
    validator: Optional[GraphLiteValidator] = None,
    progress: Optional[Callable[[str], None]] = None,
) -> Tuple[Optional[str], List[Dict[str, Any]], List[Dict[str, Any]]]:
    def notify(message: str) -> None:
        if progress:
            progress(message)

    gen = gen_model or DEFAULT_OPENAI_MODEL_GEN
    fix = fix_model or DEFAULT_OPENAI_MODEL_FIX

    graph = SchemaGraph.from_text(schema_context)
    feedback: List[str] = []
    usage_data: List[Dict[str, Any]] = []
    timeline: List[Dict[str, Any]] = []

    owns_validator = validator is None
    if validator is None:
        validator = GraphLiteValidator(db_path=DEFAULT_DB_PATH)

    try:
        for attempt in range(1, max_attempts + 1):
            notify(f"[attempt {attempt}] drafting intent frame...")
            frame, usage = draft_intent_frame(nl, graph, gen, feedback)
            if usage:
                usage.update({"attempt": attempt, "call_type": "intent_frame", "model": gen})
                usage_data.append(usage)
            timeline.append({"attempt": attempt, "action": "intent_frame", "data": frame, "feedback": feedback.copy()})

            notify(f"[attempt {attempt}] linking schema...")
            links, usage = link_schema(frame, nl, graph, gen, feedback)
            if usage:
                usage.update({"attempt": attempt, "call_type": "schema_link", "model": gen})
                usage_data.append(usage)
            timeline.append({"attempt": attempt, "action": "schema_link", "data": links})

            notify(f"[attempt {attempt}] planning AST...")
            ast, usage = plan_ast(frame, links, graph, gen, feedback)
            if usage:
                usage.update({"attempt": attempt, "call_type": "ast_plan", "model": gen})
                usage_data.append(usage)
            timeline.append({"attempt": attempt, "action": "ast_plan", "data": ast})

            # Normalize filters that incorrectly encode relationships in the property slot.
            _rewrite_filter_relationship_props(ast)

            ast_errors = validate_ast(ast, graph)
            if ast_errors:
                notify(f"[attempt {attempt}] AST invalid: {'; '.join(ast_errors)}")
                feedback.append("AST invalid: " + "; ".join(ast_errors))
                timeline.append({"attempt": attempt, "action": "ast_invalid", "errors": ast_errors})
                continue

            gql_query = render_ast_to_gql(ast, graph)
            timeline.append({"attempt": attempt, "action": "rendered_query", "query": gql_query})
            notify(f"[attempt {attempt}] rendered query; validating syntax...")

            syntax_valid, syntax_error = validator.validate(gql_query)
            timeline.append(
                {
                    "attempt": attempt,
                    "action": "syntax_check",
                    "valid": syntax_valid,
                    "error": syntax_error,
                    "query": gql_query,
                }
            )

            if not syntax_valid:
                notify(f"[attempt {attempt}] syntax failed: {syntax_error}")
                feedback.append(f"Syntax error: {syntax_error}")
                continue

            logic_valid, logic_error, logic_usage = validate_logical_correctness(
                nl, schema_context, gql_query, frame, model=fix
            )
            if logic_usage:
                logic_usage.update({"attempt": attempt, "call_type": "logic_validate", "model": fix})
                usage_data.append(logic_usage)
            timeline.append(
                {
                    "attempt": attempt,
                    "action": "logic_check",
                    "valid": logic_valid,
                    "error": logic_error,
                    "query": gql_query,
                    "frame": frame,
                }
            )

            if logic_valid:
                notify(f"[attempt {attempt}] logic valid; query complete.")
                return gql_query, usage_data, timeline

            if logic_error:
                notify(f"[attempt {attempt}] logic gap: {logic_error}")
                feedback.append(f"Logic gap: {logic_error}")

        return None, usage_data, timeline
    finally:
        if owns_validator and validator:
            validator.close()


# -----------------------------------------------------------------------------
# Reporting + CLI
# -----------------------------------------------------------------------------


def _fmt_block(text: str, indent: int = 6) -> str:
    pad = " " * indent
    return "\n".join(pad + line for line in text.splitlines())


def print_verbose_info(
    nl_query: str,
    usage_data: List[Dict[str, Any]],
    validation_log: List[Dict[str, Any]],
    max_attempts: int,
    gen_model: str,
    fix_model: str,
) -> None:
    print("\n" + "=" * 80)
    print("PIPELINE EXECUTION SUMMARY")
    print("=" * 80)
    print(f"Query: {nl_query}")
    print(f"Models: {gen_model} (gen) | {fix_model} (fix)")
    print(f"Max Attempts: {max_attempts}")

    grouped: Dict[int, List[Dict[str, Any]]] = {}
    for entry in validation_log:
        grouped.setdefault(entry["attempt"], []).append(entry)

    print("\nTimeline (per attempt):")
    for attempt in sorted(grouped):
        print("-" * 80)
        print(f"Attempt {attempt}")
        for entry in grouped[attempt]:
            action = entry.get("action")
            if action == "intent_frame":
                print("  • Intent frame")
                print(_fmt_block(json.dumps(entry.get("data"), indent=2) or "<empty>"))
            elif action == "schema_link":
                print("  • Schema linking")
                print(_fmt_block(json.dumps(entry.get("data"), indent=2) or "<empty>"))
            elif action == "ast_plan":
                print("  • AST plan")
                print(_fmt_block(json.dumps(entry.get("data"), indent=2) or "<empty>"))
            elif action == "ast_invalid":
                print("  • AST invalid")
                for err in entry.get("errors", []):
                    print(f"      - {err}")
            elif action == "rendered_query":
                print("  • Rendered query:")
                block = "\n      ".join(entry.get("query", "").splitlines() or ["<empty>"])
                print(f"      ```gql\n      {block}\n      ```")
            elif action == "syntax_check":
                status = "✓ SYNTAX VALID" if entry.get("valid") else "✗ SYNTAX INVALID"
                print(f"  • {status}")
                if entry.get("error"):
                    err_block = "\n      ".join(str(entry["error"]).splitlines())
                    print(f"      ```\n      {err_block}\n      ```")
            elif action == "logic_check":
                status = "✓ LOGIC VALID" if entry.get("valid") else "✗ LOGIC INVALID"
                print(f"  • {status}")
                if entry.get("error"):
                    err_block = "\n      ".join(str(entry["error"]).splitlines())
                    print(f"      ```\n      {err_block}\n      ```")
            else:
                print(f"  • {action}: {entry}")

    total_tokens = sum(item.get("total_tokens", 0) for item in usage_data)
    print("\nAPI Usage:")
    print(f"  Calls: {len(usage_data)}")
    print(f"  Tokens: {total_tokens}")
    print("=" * 80)


def read_text(path: str) -> str:
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read().strip()


def run_pipeline(
    nl: str,
    schema_context: str,
    *,
    max_attempts: int = 3,
    gen_model: Optional[str] = None,
    fix_model: Optional[str] = None,
    db_path: Optional[str] = DEFAULT_DB_PATH,
    verbose: bool = False,
) -> str:
    progress_fn = print if verbose else None

    with GraphLiteValidator(db_path=db_path) as validator:
        result, usage_data, validation_log = generate_isogql_with_progress(
            nl,
            schema_context,
            max_attempts=max_attempts,
            gen_model=gen_model,
            fix_model=fix_model,
            validator=validator,
            progress=progress_fn,
        )

    if verbose:
        print_verbose_info(
            nl, usage_data, validation_log, max_attempts, gen_model or DEFAULT_OPENAI_MODEL_GEN, fix_model or DEFAULT_OPENAI_MODEL_FIX
        )

    if result is None:
        raise RuntimeError("Failed to generate a valid ISO GQL query")

    return result


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Generate ISO GQL queries from natural language (schema-grounded)")
    parser.add_argument("--nl", required=True, help="Natural language request")
    parser.add_argument("--schema-file", help="Path to schema context text")
    parser.add_argument("--schema", help="Schema context as a string (overrides --schema-file)")
    parser.add_argument("--max-attempts", type=int, default=3, help="Max generation/fix attempts")
    parser.add_argument("--gen-model", help="OpenAI model for generation (default: gpt-4o-mini)")
    parser.add_argument("--fix-model", help="OpenAI model for fixes/logic validation (default: gpt-4o-mini)")
    parser.add_argument("--db-path", help="GraphLite DB path for syntax validation (defaults to temp or NL2GQL_DB_PATH)")
    parser.add_argument("--verbose", action="store_true", help="Print attempt timeline and token usage")

    args = parser.parse_args(argv)

    if args.schema is not None:
        schema_context = args.schema
    elif args.schema_file:
        schema_context = read_text(args.schema_file)
    else:
        print("error: schema context is required via --schema or --schema-file", file=sys.stderr)
        return 1

    try:
        query = run_pipeline(
            args.nl,
            schema_context,
            max_attempts=args.max_attempts,
            gen_model=args.gen_model,
            fix_model=args.fix_model,
            db_path=args.db_path or DEFAULT_DB_PATH,
            verbose=args.verbose,
        )
    except Exception as exc:
        print(f"Failed to generate query: {exc}", file=sys.stderr)
        return 1

    print(query)
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
