"""
GraphLite Python API

Python wrapper around GraphLite C FFI using ctypes.
"""

import ctypes
import json
import os
import platform
from enum import IntEnum
from typing import Dict, List, Any, Optional
from pathlib import Path


class ErrorCode(IntEnum):
    """GraphLite error codes"""
    SUCCESS = 0
    NULL_POINTER = 1
    INVALID_UTF8 = 2
    DATABASE_OPEN_ERROR = 3
    SESSION_ERROR = 4
    QUERY_ERROR = 5
    PANIC_ERROR = 6
    JSON_ERROR = 7


class GraphLiteError(Exception):
    """Exception raised for GraphLite errors"""

    def __init__(self, code: ErrorCode, message: str):
        self.code = code
        self.message = message
        super().__init__(f"GraphLite error ({code.name}): {message}")


class QueryResult:
    """Query result wrapper with convenient access methods"""

    def __init__(self, data: Dict[str, Any]):
        self._data = data
        self.variables = data.get("variables", [])
        # Flatten rows from nested structure
        raw_rows = data.get("rows", [])
        self.rows = [self._flatten_row(row) for row in raw_rows]
        self.row_count = len(self.rows)

    def _flatten_row(self, row: Dict[str, Any]) -> Dict[str, Any]:
        """Flatten a row from nested value structure to simple dict"""
        if "values" not in row:
            return row

        flattened = {}
        for key, value_wrapper in row["values"].items():
            flattened[key] = self._extract_value(value_wrapper)
        return flattened

    def _extract_value(self, value_wrapper: Any) -> Any:
        """Extract value from Rust enum wrapper like {'String': 'foo'} or {'Number': 42}"""
        if not isinstance(value_wrapper, dict):
            return value_wrapper

        # Handle Rust enum variants
        if "String" in value_wrapper:
            return value_wrapper["String"]
        elif "Number" in value_wrapper:
            num = value_wrapper["Number"]
            # Convert to int if it's a whole number
            return int(num) if isinstance(num, float) and num.is_integer() else num
        elif "Boolean" in value_wrapper:
            return value_wrapper["Boolean"]
        elif "Null" in value_wrapper:
            return None
        elif "List" in value_wrapper:
            return [self._extract_value(v) for v in value_wrapper["List"]]
        elif "Map" in value_wrapper:
            return {k: self._extract_value(v) for k, v in value_wrapper["Map"].items()}
        elif "Node" in value_wrapper:
            return value_wrapper  # Return node reference as-is
        elif "Edge" in value_wrapper:
            return value_wrapper  # Return edge reference as-is
        elif "Path" in value_wrapper:
            return value_wrapper  # Return path as-is
        else:
            return value_wrapper

    def __repr__(self):
        return f"QueryResult(rows={self.row_count}, variables={self.variables})"

    def to_dict(self) -> Dict[str, Any]:
        """Get raw dictionary representation"""
        return self._data

    def first(self) -> Optional[Dict[str, Any]]:
        """Get first row or None"""
        return self.rows[0] if self.rows else None

    def column(self, name: str) -> List[Any]:
        """Get all values from a specific column"""
        return [row.get(name) for row in self.rows]


def _find_library() -> str:
    """Find the GraphLite shared library"""
    system = platform.system()

    # Determine library name based on platform
    if system == "Darwin":  # macOS
        lib_name = "libgraphlite_ffi.dylib"
    elif system == "Windows":
        lib_name = "graphlite_ffi.dll"
    else:  # Linux and others
        lib_name = "libgraphlite_ffi.so"

    # Search paths
    search_paths = [
        # Relative to this file (development)
        Path(__file__).parent.parent.parent.parent / "target" / "release" / lib_name,
        Path(__file__).parent.parent.parent.parent / "target" / "debug" / lib_name,
        # Installed location
        Path("/usr/local/lib") / lib_name,
        Path("/usr/lib") / lib_name,
        # Current directory
        Path.cwd() / lib_name,
    ]

    for path in search_paths:
        if path.exists():
            return str(path)

    raise FileNotFoundError(
        f"Could not find GraphLite library ({lib_name}). "
        f"Please build the FFI library first: cargo build --release -p graphlite-ffi"
    )


# Load the library
_lib_path = _find_library()
_lib = ctypes.CDLL(_lib_path)

# Define C structures
class _GraphLiteDB(ctypes.Structure):
    """Opaque database handle"""
    pass


# Define function signatures
_lib.graphlite_open.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_int)]
_lib.graphlite_open.restype = ctypes.POINTER(_GraphLiteDB)

_lib.graphlite_create_session.argtypes = [
    ctypes.POINTER(_GraphLiteDB),
    ctypes.c_char_p,
    ctypes.POINTER(ctypes.c_int)
]
_lib.graphlite_create_session.restype = ctypes.c_void_p

_lib.graphlite_query.argtypes = [
    ctypes.POINTER(_GraphLiteDB),
    ctypes.c_char_p,
    ctypes.c_char_p,
    ctypes.POINTER(ctypes.c_int)
]
_lib.graphlite_query.restype = ctypes.c_void_p

_lib.graphlite_close_session.argtypes = [
    ctypes.POINTER(_GraphLiteDB),
    ctypes.c_char_p,
    ctypes.POINTER(ctypes.c_int)
]
_lib.graphlite_close_session.restype = ctypes.c_int

_lib.graphlite_free_string.argtypes = [ctypes.c_void_p]
_lib.graphlite_free_string.restype = None

_lib.graphlite_close.argtypes = [ctypes.POINTER(_GraphLiteDB)]
_lib.graphlite_close.restype = None

_lib.graphlite_version.argtypes = []
_lib.graphlite_version.restype = ctypes.c_void_p


class GraphLite:
    """
    GraphLite database connection

    Example:
        >>> db = GraphLite("./mydb")
        >>> session = db.create_session("admin")
        >>> result = db.query(session, "MATCH (n) RETURN n")
        >>> print(result.rows)
        >>> db.close()

    Or use as context manager:
        >>> with GraphLite("./mydb") as db:
        ...     session = db.create_session("admin")
        ...     result = db.query(session, "MATCH (n) RETURN n")
    """

    def __init__(self, path: str):
        """
        Open a GraphLite database

        Args:
            path: Path to database directory

        Raises:
            GraphLiteError: If database cannot be opened
        """
        self._db = None
        self._sessions = set()

        error = ctypes.c_int(0)
        self._db = _lib.graphlite_open(path.encode('utf-8'), ctypes.byref(error))

        if not self._db:
            raise GraphLiteError(
                ErrorCode(error.value),
                f"Failed to open database at {path}"
            )

    def create_session(self, username: str) -> str:
        """
        Create a new session for the given user

        Args:
            username: Username for the session

        Returns:
            Session ID string

        Raises:
            GraphLiteError: If session creation fails
        """
        if not self._db:
            raise GraphLiteError(ErrorCode.NULL_POINTER, "Database is closed")

        error = ctypes.c_int(0)
        session_id_ptr = _lib.graphlite_create_session(
            self._db,
            username.encode('utf-8'),
            ctypes.byref(error)
        )

        if not session_id_ptr:
            raise GraphLiteError(
                ErrorCode(error.value),
                f"Failed to create session for user '{username}'"
            )

        # Copy the string before freeing
        session_id = ctypes.string_at(session_id_ptr).decode('utf-8')
        _lib.graphlite_free_string(session_id_ptr)
        self._sessions.add(session_id)

        return session_id

    def query(self, session_id: str, query: str) -> QueryResult:
        """
        Execute a GQL query

        Args:
            session_id: Session ID from create_session()
            query: GQL query string

        Returns:
            QueryResult with rows and metadata

        Raises:
            GraphLiteError: If query execution fails
        """
        if not self._db:
            raise GraphLiteError(ErrorCode.NULL_POINTER, "Database is closed")

        error = ctypes.c_int(0)
        result_ptr = _lib.graphlite_query(
            self._db,
            session_id.encode('utf-8'),
            query.encode('utf-8'),
            ctypes.byref(error)
        )

        if not result_ptr:
            raise GraphLiteError(
                ErrorCode(error.value),
                f"Query failed: {query[:100]}"
            )

        try:
            # Copy the string before freeing
            result_json = ctypes.string_at(result_ptr).decode('utf-8')
            result_data = json.loads(result_json)
            return QueryResult(result_data)
        except json.JSONDecodeError as e:
            raise GraphLiteError(ErrorCode.JSON_ERROR, f"Invalid JSON response: {e}")
        finally:
            _lib.graphlite_free_string(result_ptr)

    def execute(self, session_id: str, statement: str) -> None:
        """
        Execute a statement without returning results

        Args:
            session_id: Session ID from create_session()
            statement: GQL statement to execute

        Raises:
            GraphLiteError: If execution fails
        """
        self.query(session_id, statement)

    def close_session(self, session_id: str) -> None:
        """
        Close a session

        Args:
            session_id: Session ID to close

        Raises:
            GraphLiteError: If session close fails
        """
        if not self._db:
            return

        error = ctypes.c_int(0)
        result = _lib.graphlite_close_session(
            self._db,
            session_id.encode('utf-8'),
            ctypes.byref(error)
        )

        if result != 0:
            raise GraphLiteError(
                ErrorCode(error.value),
                f"Failed to close session {session_id}"
            )

        self._sessions.discard(session_id)

    def close(self) -> None:
        """Close the database and all sessions"""
        if self._db:
            # Close all open sessions
            for session_id in list(self._sessions):
                try:
                    self.close_session(session_id)
                except GraphLiteError:
                    pass  # Ignore errors during cleanup

            _lib.graphlite_close(self._db)
            self._db = None

    def __enter__(self):
        """Context manager entry"""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit"""
        self.close()
        return False

    def __del__(self):
        """Destructor"""
        self.close()

    @staticmethod
    def version() -> str:
        """Get GraphLite version"""
        version_ptr = _lib.graphlite_version()
        if version_ptr:
            # Don't free - version() returns a static string
            version = ctypes.string_at(version_ptr).decode('utf-8')
            return version
        return "unknown"