from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Sequence, Tuple

from .config import DEFAULT_OPENAI_MODEL_GEN
from .openai_client import chat_complete, safe_json_loads
from .schema_graph import SchemaGraph
from .utils import canonical, ratio


@dataclass
class IntentLinkGuidance:
    frame: Dict[str, Any]
    links: Dict[str, Any]


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


def draft_intent_frame(nl: str, schema_text: str, model: str, feedback: List[str]) -> Tuple[Dict[str, Any], Optional[Dict[str, Any]]]:
    user = USER_INTENT_TEMPLATE.format(graph=schema_text, nl=nl)
    if feedback:
        user += "\n\nprevious_failures:\n- " + "\n- ".join(feedback[-5:])

    text, usage = chat_complete(model, SYSTEM_INTENT, user, temperature=0.0, top_p=0.2)
    frame = safe_json_loads(text) or {}
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

    text, usage = chat_complete(model, SYSTEM_LINK, user, temperature=0.0, top_p=0.2)
    links = safe_json_loads(text) or {}
    return links, usage


def _closest_schema_label(raw_label: str, alias: str, property_links: List[Dict[str, Any]], graph: SchemaGraph) -> Optional[str]:
    canonical_label = canonical(raw_label)
    props_for_alias = {
        canonical(pl["property"])
        for pl in property_links
        if pl.get("alias") == alias and pl.get("property")
    }

    best_label: Optional[str] = None
    best_score = 0.0
    for schema_label, node in graph.nodes.items():
        score = ratio(canonical_label, canonical(schema_label))
        if props_for_alias:
            overlap = props_for_alias & {canonical(p) for p in node.properties}
            if overlap:
                score += 0.8 + 0.3 * len(overlap)
        if score > best_score:
            best_score = score
            best_label = schema_label

    return best_label if best_score >= 0.55 else None


def _closest_property(label: str, prop: str, graph: SchemaGraph) -> Optional[str]:
    if not graph.has_node(label):
        return None

    canonical_prop = canonical(prop)
    best_prop: Optional[str] = None
    best_score = 0.0
    for candidate in graph.nodes[label].properties:
        score = ratio(canonical_prop, canonical(candidate))
        if score > best_score:
            best_score = score
            best_prop = candidate
    return best_prop if best_score >= 0.75 else None


def _closest_relationship(left_label: str, raw_rel: str, right_label: str, graph: SchemaGraph) -> Optional[str]:
    candidates = [e for e in graph.edges if e.src == left_label and e.dst == right_label]
    if not candidates:
        return None

    canonical_rel = canonical(raw_rel)
    best: Optional[str] = None
    best_score = 0.0
    for edge in candidates:
        score = ratio(canonical_rel, canonical(edge.rel))
        if score > best_score:
            best_score = score
            best = edge.rel
    return best if best_score >= 0.6 else None


def ground_links_to_schema(links: Dict[str, Any], graph: SchemaGraph) -> Dict[str, Any]:
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

    return {
        "node_links": grounded_nodes,
        "property_links": grounded_props,
        "rel_links": grounded_rels,
        "canonical_aliases": links.get("canonical_aliases", {}),
    }


def links_to_hints(links: Dict[str, Any]) -> List[str]:
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

    def run(self, nl: str, pre, failures: List[str]) -> IntentLinkGuidance:
        frame, _ = draft_intent_frame(nl, pre.filtered_schema.describe(), self.model, failures)
        hits = (
            pre.filtered_schema.strategy_hits.get("exact", [])
            + pre.filtered_schema.strategy_hits.get("semantic", [])
            + pre.filtered_schema.strategy_hits.get("ner_masked", [])
        )
        links_raw, _ = link_schema(frame, nl, pre.filtered_schema.describe(), self.model, failures, heuristic_hits=hits)
        grounded = ground_links_to_schema(links_raw, self.graph)
        return IntentLinkGuidance(frame=frame, links=grounded)


__all__ = [
    "IntentLinker",
    "IntentLinkGuidance",
    "draft_intent_frame",
    "link_schema",
    "ground_links_to_schema",
    "links_to_hints",
]



