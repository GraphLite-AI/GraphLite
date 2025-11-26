'''
Result handling and typed deserialization

This module provides utilities for working with query results, including
type-safe deserialization into Rust structs.
'''

from typing import Optional, List, Any, Dict, TypeVar, Type, TYPE_CHECKING
from .error import SerializationError

if TYPE_CHECKING:
    from .connection import Session

import sys
from pathlib import Path

# Add bindings/python to path so we can import the low-level bindings
bindings_path = Path(__file__).parent.parent.parent / "bindings" / "python"
if str(bindings_path) not in sys.path:
    sys.path.insert(0, str(bindings_path))

from graphlite import QueryResult

T = TypeVar('T')

class TypedResult:
    '''
    Wrapper around QueryResult with additional type-safe methods

    TypedResult provides convenient methods for deserializing query results
    into Python types.
    '''

    def __init__(self, result: QueryResult):
        '''
        Create a new TypedResult from a QueryResult
        '''
        self.result = result

    def row_count(self) -> int:
        '''
        Get the number of rows in the result
        '''
        return len(self.result.rows)

    def column_names(self) -> List[str]:
        '''
        Get the column names (variables) in the result
        '''
        return self.result.variables

    def get_row(self, index: int) -> Optional[Dict[str, Any]]:
        '''
        Get a specific row by index
        '''
        if 0 <= index < len(self.result.rows):
            return self.result.rows[index]
        return None

    def deserialize_rows(self, target_type: Type[T]) -> List[T]:
        '''
        Deserialize all rows into instances of the target type

        Args:
            target_type: A class/type to deserialize into (e.g., a dataclass)

        Returns:
            List of deserialized objects

        Raises:
            SerializationError: If deserialization fails

        Examples:
            >>> @dataclass
            >>> class Person:
            ...     name: str
            ...     age: int
            >>>
            >>> result = session.query("MATCH (p:Person) RETURN p.name as name, p.age as age")
            >>> typed = TypedResult(result)
            >>> people = typed.deserialize_rows(Person)
        '''
        try:
            objects = []
            for row in self.result.rows:
                obj = self.deserialize_row(row, target_type)
                objects.append(obj)
            return objects
        except Exception as e:
            raise SerializationError(f"Failed to deserialize rows: {e}")

    def deserialize_row(self, row: Dict[str, Any], target_type: Type[T]) -> T:
        '''
        Deserialize a single row dict into an instance of the target type

        Args:
            row: Row dictionary to deserialize
            target_type: A class/type to deserialize into

        Returns:
            Deserialized object

        Raises:
            SerializationError: If deserialization fails

        Examples:
            >>> row = result.rows[0]
            >>> first_person = typed.deserialize_row(row, Person)
        '''
        try:
            return target_type(**row)
        except Exception as e:
            raise SerializationError(f"Failed to deserialize row: {e}")

    def first(self, target_type: Type[T]) -> T:
        '''
        Get the first row as the given type

        Convenience method for queries that return a single row.

        Args:
            target_type: A class/type to deserialize into

        Returns:
            Deserialized object from the first row

        Raises:
            SerializationError: If no rows exist or deserialization fails

        Examples:
            >>> @dataclass
            >>> class Count:
            ...     count: int
            >>>
            >>> result = session.query("MATCH (p:Person) RETURN count(p) as count")
            >>> typed = TypedResult(result)
            >>> count_obj = typed.first(Count)
        '''
        if self.is_empty():
            raise SerializationError("No rows returned")

        return self.deserialize_row(self.result.rows[0], target_type)

    def scalar(self) -> Any:
        '''
        Get a single value from the first row and first column

        Useful for queries that return a single scalar value.

        Returns:
            The scalar value from the first row, first column

        Raises:
            SerializationError: If no rows or columns exist

        Examples:
            >>> result = session.query("MATCH (p:Person) RETURN count(p)")
            >>> typed = TypedResult(result)
            >>> count = typed.scalar()  # Returns the count value directly
        '''
        if self.is_empty():
            raise SerializationError("No rows returned")

        columns = self.result.variables
        if not columns:
            raise SerializationError("No columns returned")

        first_row = self.result.rows[0]
        first_column = columns[0]

        if first_column not in first_row:
            raise SerializationError(f"Column '{first_column}' not found in row")

        return first_row[first_column]

    def is_empty(self) -> bool:
        '''
        Check if the result is empty (no rows)

        Returns:
            True if result has no rows, False otherwise
        '''
        return len(self.result.rows) == 0

    def rows(self) -> List[Dict[str, Any]]:
        '''
        Get all rows

        Returns:
            List of row dictionaries
        '''
        return self.result.rows


__all__ = ['TypedResult']