from __future__ import annotations

import re
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Set, Tuple

from .utils import canonical, ratio, tokenize


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
            rel_match = re.match(
                r"^\(?([A-Za-z0-9_]+)\)?-?\s*\[:([A-Za-z0-9_]+)\]\s*->\s*\(?([A-Za-z0-9_]+)\)?", line
            )
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
            adjacency[edge.dst]
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
        canon_term = canonical(term)
        for item in pool:
            score = ratio(canon_term, canonical(item))
            if score > 0.0:
                scored.append((item, score))
        scored.sort(key=lambda x: x[1], reverse=True)
        return scored[:limit]

    def shortest_paths(self, starts: Set[str], targets: Set[str], max_depth: int = 3) -> List[List[SchemaEdge]]:
        if not starts or not targets:
            return []
        paths: List[List[SchemaEdge]] = []
        from collections import deque

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
        node_lines = [
            f"- {n.name}: {', '.join(n.properties) if n.properties else 'no properties listed'}" for n in nodes.values()
        ]
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



