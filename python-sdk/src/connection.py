"""
Database connection and session management
This module provides the main entry points for working with GraphLite databases.
"""

import sys
from pathlib import Path

# Add bindings/python to path so we can import the low-level bindings
bindings_path = Path(__file__).parent.parent.parent / "bindings" / "python"
if str(bindings_path) not in sys.path:
    sys.path.insert(0, str(bindings_path))

# Low-level GraphLite binding (like QueryCoordinator in Rust)
from graphlite import GraphLite as _GraphLiteBinding, QueryResult
from .error import ConnectionError, SessionError, QueryError


class GraphLite:
    """
    GraphLite database connection

    High-level wrapper around the FFI bindings, similar to the Rust SDK.
    """

    def __init__(self, db: _GraphLiteBinding):
        """
        Internal constructor - use GraphLite.open() instead
        
        Store instance of the low-level GraphLite binding
        """
        self._db = db

    @classmethod
    def open(cls, path: str):
        """
        Open a GraphLite database at the given path

        Args:
            path: Path to the database directory

        Returns:
            GraphLite instance

        Raises:
            ConnectionError: If database cannot be opened
        """
        try:
            db = _GraphLiteBinding(path)
            return cls(db)
        except Exception as e:
            raise ConnectionError(f"Failed to open database: {e}")

    def session(self, username: str):
        """
        Create a new session for the given user

        Args:
            username: Username for the session

        Returns:
            Session instance

        Raises:
            SessionError: If session creation fails
        """
        try:
            session_id = self._db.create_session(username)
            return Session(session_id, self._db, username)
        except Exception as e:
            raise SessionError(f"Failed to create session: {e}")

    def close(self):
        """Close the database connection"""
        if self._db:
            self._db.close()


class Session:
    """
    GraphLite database session

    Provides user context for executing queries.
    """

    def __init__(self, session_id: str, db: _GraphLiteBinding, username: str):
        """Internal constructor - use db.session() instead"""
        self._session_id = session_id
        self._db = db
        self._username = username

    def id(self) -> str:
        """Get the session ID"""
        return self._session_id

    def username(self) -> str:
        """Get the username for this session"""
        return self._username

    def query(self, query: str) -> QueryResult:
        """
        Execute a GQL query

        Args:
            query: GQL query string

        Returns:
            QueryResult with rows and metadata

        Raises:
            QueryError: If query execution fails
        """
        try:
            return self._db.query(self._session_id, query)
        except Exception as e:
            raise QueryError(f"Query failed: {e}")

    def execute(self, statement: str):
        """
        Execute a statement without returning results

        Args:
            statement: GQL statement to execute

        Raises:
            QueryError: If execution fails
        """
        try:
            self._db.execute(self._session_id, statement)
        except Exception as e:
            raise QueryError(f"Execute failed: {e}")

    def transaction(self):
        """
        Begin a new transaction

        Returns a Transaction object that should be used as a context manager.
        The transaction will automatically roll back when the context exits
        unless commit() is explicitly called.

        Returns:
            Transaction instance

        Raises:
            TransactionError: If transaction cannot be started
        """
        from .transaction import Transaction
        return Transaction(self)

    def query_builder(self):
        """
        Create a new query builder for fluent query construction

        Returns:
            QueryBuilder instance
        """
        from .query import QueryBuilder
        return QueryBuilder(self)


__all__ = ['GraphLite', 'Session', 'QueryResult']
