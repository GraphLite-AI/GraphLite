from __future__ import annotations

import os
from pathlib import Path
from typing import Optional

try:
    from dotenv import load_dotenv
except ImportError:  # pragma: no cover
    load_dotenv = None  # type: ignore

_ENV_PATH = Path(__file__).resolve().parents[1] / "config.env"

if load_dotenv:
    if _ENV_PATH.exists():
        load_dotenv(_ENV_PATH)
    else:
        load_dotenv()

DEFAULT_OPENAI_MODEL_GEN = os.getenv("OPENAI_MODEL_GEN", "gpt-4o-mini")
DEFAULT_OPENAI_MODEL_FIX = os.getenv("OPENAI_MODEL_FIX", "gpt-4o-mini")

DEFAULT_DB_PATH: Optional[str] = os.getenv("NL2GQL_DB_PATH")
DEFAULT_DB_USER = os.getenv("NL2GQL_USER", "admin")
DEFAULT_DB_SCHEMA = os.getenv("NL2GQL_SCHEMA", "nl2gql")
DEFAULT_DB_GRAPH = os.getenv("NL2GQL_GRAPH", "scratch")



