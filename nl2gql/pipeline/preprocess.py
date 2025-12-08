from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Dict, List, Set, Tuple

from .schema_graph import FilteredSchema, SchemaEdge, SchemaGraph, SchemaNode
from .utils import canonical, ratio, tokenize


@dataclass
class PreprocessResult:
    raw_nl: str
    normalized_nl: str
    phrases: List[str]
    filtered_schema: FilteredSchema
    structural_hints: List[str]


class Preprocessor:
    def __init__(self, graph: SchemaGraph) -> None:
        self.graph = graph

    def _normalize(self, text: str) -> str:
        text = text.strip()
        text = re.sub(r"\s+", " ", text)
        return text

    def _extract_phrases(self, text: str) -> List[str]:
        tokens = tokenize(text)
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
        canon_tokens = {canonical(t) for t in tokens}
        for label in self.graph.nodes:
            if canonical(label) in canon_tokens:
                hits.add(label)
        for prop in {p for n in self.graph.nodes.values() for p in n.properties}:
            if canonical(prop) in canon_tokens:
                hits.add(prop)
        for rel in {e.rel for e in self.graph.edges}:
            if canonical(rel) in canon_tokens:
                hits.add(rel)
        for phrase in phrases:
            canon = canonical(phrase)
            for label in self.graph.nodes:
                if canon == canonical(label):
                    hits.add(label)
        return hits

    def _strategy_ner_mask(self, text: str) -> Set[str]:
        masked = self._mask_entities(text)
        tokens = {canonical(t) for t in tokenize(masked)}
        hits: Set[str] = set()
        for label in self.graph.nodes:
            if canonical(label) in tokens:
                hits.add(label)
        for rel in {e.rel for e in self.graph.edges}:
            if canonical(rel) in tokens:
                hits.add(rel)
        for prop in {p for n in self.graph.nodes.values() for p in n.properties}:
            if canonical(prop) in tokens:
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

        for edge in edges:
            if edge.src in self.graph.nodes:
                candidate_nodes.setdefault(edge.src, self.graph.nodes[edge.src])
            if edge.dst in self.graph.nodes:
                candidate_nodes.setdefault(edge.dst, self.graph.nodes[edge.dst])

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
        tokens = set(tokenize(normalized))

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


