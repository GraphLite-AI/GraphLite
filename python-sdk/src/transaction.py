"""
Transaction support with ACID guarantees

This module provides transaction support following the rusqlite pattern:
- Transactions automatically roll back when context exits (unless committed)
- Explicit commit() required to persist changes
- Can be used as a context manager for automatic cleanup
"""

from typing import TYPE_CHECKING
from .error import TransactionError

if TYPE_CHECKING:
    from .connection import Session

import sys
from pathlib import Path
bindings_path = Path(__file__).parent.parent.parent / "bindings" / "python"
if str(bindings_path) not in sys.path:
    sys.path.insert(0, str(bindings_path))

# Import QueryResult for type hints
from graphlite import QueryResult


class Transaction:
    """
    Represents an active database transaction

    Transactions provide ACID guarantees for multi-statement operations.
    Following the rusqlite pattern:
    - Transactions automatically roll back when context exits
    - Must explicitly call commit() to persist changes
    - This prevents accidentally forgetting to commit or rollback

    Examples:
        >>> db = GraphLite.open("./mydb")
        >>> session = db.session("admin")
        >>>
        >>> # Using as context manager (recommended)
        >>> with session.transaction() as tx:
        ...     tx.execute("INSERT (p:Person {name: 'Alice'})")
        ...     tx.execute("INSERT (p:Person {name: 'Bob'})")
        ...     tx.commit()  # Changes are persisted
        >>>
        >>> # Transaction that rolls back (no commit)
        >>> with session.transaction() as tx:
        ...     tx.execute("INSERT (p:Person {name: 'Charlie'})")
        ...     # Automatically rolled back on context exit
    """

    def __init__(self, session: 'Session'):
        """
        Internal constructor - use session.transaction() instead

        Begin a new transaction
        """
        self._session = session
        self._committed = False
        self._rolled_back = False

        # Execute BEGIN TRANSACTION
        try:
            self._session._db.execute(self._session._session_id, "BEGIN TRANSACTION")
        except Exception as e:
            raise TransactionError(f"Failed to begin transaction: {e}")

    def execute(self, statement: str) -> None:
        """
        Execute a GQL statement within this transaction

        Args:
            statement: GQL statement to execute

        Raises:
            TransactionError: If transaction is already finished or execution fails

        Examples:
            >>> with session.transaction() as tx:
            ...     tx.execute("INSERT (p:Person {name: 'Alice'})")
            ...     tx.execute("INSERT (p:Person {name: 'Bob'})")
            ...     tx.commit()
        """
        if self._committed:
            raise TransactionError("Transaction already committed")
        if self._rolled_back:
            raise TransactionError("Transaction already rolled back")

        try:
            self._session._db.execute(self._session._session_id, statement)
        except Exception as e:
            raise TransactionError(f"Execute failed: {e}")

    def query(self, query: str) -> QueryResult:
        """
        Execute a query within this transaction and return results

        Args:
            query: GQL query to execute

        Returns:
            QueryResult with rows and metadata

        Raises:
            TransactionError: If transaction is already finished or query fails

        Examples:
            >>> with session.transaction() as tx:
            ...     tx.execute("INSERT (p:Person {name: 'Alice', age: 30})")
            ...     result = tx.query("MATCH (p:Person) RETURN p")
            ...     tx.commit()
        """
        if self._committed:
            raise TransactionError("Transaction already committed")
        if self._rolled_back:
            raise TransactionError("Transaction already rolled back")

        try:
            return self._session._db.query(self._session._session_id, query)
        except Exception as e:
            raise TransactionError(f"Query failed: {e}")

    def commit(self) -> None:
        """
        Commit the transaction

        Persists all changes made within this transaction. After calling commit(),
        the transaction cannot be used further.

        Raises:
            TransactionError: If transaction is already finished or commit fails

        Examples:
            >>> with session.transaction() as tx:
            ...     tx.execute("INSERT (p:Person {name: 'Alice'})")
            ...     tx.commit()  # Changes are now persistent
        """
        if self._committed:
            raise TransactionError("Transaction already committed")
        if self._rolled_back:
            raise TransactionError("Transaction already rolled back")

        try:
            self._session._db.execute(self._session._session_id, "COMMIT")
            self._committed = True
        except Exception as e:
            raise TransactionError(f"Failed to commit: {e}")

    def rollback(self) -> None:
        """
        Rollback the transaction

        Discards all changes made within this transaction. This is called
        automatically when the transaction context exits, so explicit rollback
        is rarely needed.

        Raises:
            TransactionError: If rollback fails

        Examples:
            >>> with session.transaction() as tx:
            ...     tx.execute("INSERT (p:Person {name: 'Alice'})")
            ...     tx.rollback()  # Explicit rollback (optional, automatic on exit)
        """
        if self._committed:
            return  # Already committed, nothing to rollback
        if self._rolled_back:
            return  # Already rolled back

        try:
            self._session._db.execute(self._session._session_id, "ROLLBACK")
            self._rolled_back = True
        except Exception as e:
            raise TransactionError(f"Failed to rollback: {e}")

    def __enter__(self):
        """Context manager entry"""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """
        Context manager exit - automatically rollback if not committed

        If an exception occurred, always rollback.
        If no exception, rollback unless commit() was called.
        """
        if exc_type is not None:
            # Exception occurred, always rollback
            if not self._committed and not self._rolled_back:
                try:
                    self.rollback()
                except Exception:
                    pass  # Ignore rollback errors during exception handling
        else:
            # No exception - rollback if not committed
            if not self._committed and not self._rolled_back:
                try:
                    self.rollback()
                except Exception:
                    pass  # Ignore rollback errors

        return False  # Don't suppress exceptions


__all__ = ['Transaction']
