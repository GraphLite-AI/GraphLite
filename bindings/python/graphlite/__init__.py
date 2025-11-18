"""
GraphLite Python Bindings

High-level Python API for GraphLite graph database using FFI.
"""

from .graphlite import GraphLite, GraphLiteError, ErrorCode, QueryResult

__version__ = "0.1.0"
__all__ = ["GraphLite", "GraphLiteError", "ErrorCode", "QueryResult"]
