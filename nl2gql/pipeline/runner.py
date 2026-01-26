from __future__ import annotations

import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Optional

from .config import DEFAULT_DB_GRAPH, DEFAULT_DB_PATH, DEFAULT_DB_SCHEMA, DEFAULT_DB_USER


def _load_graphlite_sdk():
    """Import GraphLite bindings lazily to avoid import-time failures."""

    try:
        from graphlite import GraphLite, GraphLiteError  # type: ignore

        if getattr(GraphLite, "__name__", None):
            return GraphLite, GraphLiteError
    except Exception:
        pass

    bindings_path = Path(__file__).resolve().parents[2] / "bindings" / "python"
    if bindings_path.exists():
        import sys

        sys.path.insert(0, str(bindings_path))
        sys.modules.pop("graphlite", None)
        from graphlite import GraphLite, GraphLiteError  # type: ignore

        return GraphLite, GraphLiteError

    raise SystemExit(
        "GraphLite Python bindings are missing. Build the FFI and install with: "
        "cargo build -p graphlite-ffi --release && pip install -e bindings/python"
    )


GraphLite = None
GraphLiteError = None


def _ensure_graphlite_loaded():
    global GraphLite, GraphLiteError
    if GraphLite is None or GraphLiteError is None:
        GraphLite, GraphLiteError = _load_graphlite_sdk()


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
        self._db = None
        self._session: Optional[str] = None
        self._ready = False

    def __enter__(self) -> "GraphLiteRunner":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def _ensure_ready(self) -> None:
        if self._ready:
            return
        _ensure_graphlite_loaded()
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
                    except Exception:
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
        # Wrap relationship names in backticks to avoid reserved-word collisions (e.g., FOR_PRODUCT).
        import re

        def _wrap_rel(match: re.Match) -> str:
            rel = match.group(1)
            if rel.startswith("`") and rel.endswith("`"):
                return match.group(0)
            return f"-[:`{rel}`]"

        query_text = re.sub(r"-\[:([A-Za-z0-9_]+)\]", _wrap_rel, query.strip())
        try:
            self._ensure_ready()
            assert self._db is not None and self._session is not None
            rows = self._db.query(self._session, query_text)
            return SyntaxResult(ok=True, rows=rows)
        except Exception as exc:  # pragma: no cover
            message = getattr(exc, "message", None) or str(exc)
            return SyntaxResult(ok=False, error=message)


__all__ = ["GraphLiteRunner", "SyntaxResult"]


