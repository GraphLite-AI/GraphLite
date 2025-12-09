from __future__ import annotations

from typing import Optional, Tuple

from .config import DEFAULT_DB_PATH, DEFAULT_OPENAI_MODEL_FIX, DEFAULT_OPENAI_MODEL_GEN
from .generator import QueryGenerator
from .intent_linker import IntentLinker
from .preprocess import Preprocessor
from .refiner import Refiner
from .schema_graph import SchemaGraph
from .validators import LogicValidator


class NL2GQLPipeline:
    def __init__(
        self,
        schema_context: str,
        *,
        gen_model: str = DEFAULT_OPENAI_MODEL_GEN,
        fix_model: str = DEFAULT_OPENAI_MODEL_FIX,
        db_path: Optional[str] = DEFAULT_DB_PATH,
        max_refinements: int = 2,
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

    def run(self, nl: str, *, spinner=None, trace_path: Optional[str] = None) -> Tuple[str, list]:
        return self.refiner.run(nl, self.preprocessor, self.intent_linker, spinner, trace_path=trace_path)


__all__ = ["NL2GQLPipeline"]



