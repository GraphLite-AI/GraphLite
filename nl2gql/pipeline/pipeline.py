from __future__ import annotations

from typing import Optional, Tuple

from .config import DEFAULT_DB_PATH, DEFAULT_OPENAI_MODEL_FIX, DEFAULT_OPENAI_MODEL_GEN
from .generator import QueryGenerator
from .intent_linker import IntentLinker
from .preprocess import Preprocessor
from .refiner import PipelineFailure, Refiner
from .run_logger import DEFAULT_LOG_RETAIN, RunLogger
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
        log_dir: Optional[str] = None,
        log_retain: int = DEFAULT_LOG_RETAIN,
    ) -> None:
        self.schema_context = schema_context
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
        self.log_dir = log_dir
        self.log_retain = log_retain
        self.last_run_logger: Optional[RunLogger] = None

    def run(
        self,
        nl: str,
        *,
        spinner=None,
        trace_path: Optional[str] = None,
        run_logger: Optional[RunLogger] = None,
    ) -> Tuple[str, list]:
        logger = run_logger or RunLogger(base_dir=trace_path or self.log_dir, retain=self.log_retain)
        self.last_run_logger = logger
        logger.start(
            nl,
            self.schema_context,
            {
                "gen_model": getattr(self.generator, "model", None),
                "fix_model": getattr(self.refiner.logic_validator, "model", None),
                "max_refinements": self.refiner.max_loops,
                "db_path": getattr(self.refiner.runner, "_db_path", None),
            },
        )
        trace_destination = str(logger.trace_dir) if logger.trace_dir else trace_path

        try:
            query, timeline = self.refiner.run(
                nl,
                self.preprocessor,
                self.intent_linker,
                spinner,
                trace_path=trace_destination,
                run_logger=logger,
            )
            logger.log_timeline(nl, timeline, self.refiner.max_loops)
            logger.finalize("success", {"query": query})
            return query, timeline
        except PipelineFailure as exc:
            logger.log_timeline(nl, exc.timeline, self.refiner.max_loops)
            logger.finalize("failure", {"error": str(exc), "failures": exc.failures})
            raise
        except Exception as exc:
            logger.finalize("error", {"error": str(exc)})
            raise


__all__ = ["NL2GQLPipeline"]

