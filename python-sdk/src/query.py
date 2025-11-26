'''
Query builder for fluent GQL query construction

This module provides a builder API for constructing GQL queries in a
type-safe and ergonomic way.
'''

from typing import Optional, List, TYPE_CHECKING
from .error import QueryError

if TYPE_CHECKING:
    from .connection import Session

import sys
from pathlib import Path
bindings_path = Path(__file__).parent.parent.parent / "bindings" / "python"
if str(bindings_path) not in sys.path:
    sys.path.insert(0, str(bindings_path))

class QueryBuilder:
    '''
    Fluent API for building GQL queries

    QueryBuilder provides a convenient way to construct complex GQL queries
    without manually concatenating strings.
    '''

    def __init__(self, session: 'Session'):
        '''
        Initialize the query builder
        '''
        self._session = session
        self._match_patterns = []
        self._where_clauses = []
        self._with_clauses = []
        self._return_clause = None
        self._order_by = None
        self._skip = None
        self._limit = None
    
    def match_pattern(self, pattern: str) -> 'QueryBuilder':
        '''
        Add a MATCH pattern to the query
        Can be called multiple times to add multiple MATCH patterns.
        '''
        self._match_patterns.append(pattern)
        return self
    
    def where_clause(self, condition: str) -> 'QueryBuilder':
        '''
        Add a WHERE clause to the query
        Can be called multiple times - conditions are AND'ed together.
        '''
        self._where_clauses.append(condition)
        return self
    
    def with_clause(self, clause: str) -> 'QueryBuilder':
        '''
        Add a WITH clause to the query
        WITH clauses are used for query chaining and intermediate results.
        '''
        self._with_clauses.append(clause)
        return self
    
    def return_clause(self, clause: str) -> 'QueryBuilder':
        '''
        Set the RETURN clause to the query
        Specifies what to return from the query. Required for MATCH queries.
        '''
        self._return_clause = clause
        return self
    
    def order_by(self, clause: str) -> 'QueryBuilder':
        '''
        Set the ORDER BY clause to the query
        '''
        self._order_by = clause
        return self
    
    def skip(self, n: int) -> 'QueryBuilder':
        '''
        Set the SKIP value to the query
        Skips the first N results.
        '''
        self._skip = n
        return self
    
    def limit(self, n: int) -> 'QueryBuilder':
        '''
        Set the LIMIT value to the query
        Limits the number of results returned.
        '''
        self._limit = n
        return self

    def build(self) -> str:
        '''
        Build the query string without executing
        Returns the constructed GQL query as a string.
        '''
        query = ""

        # MATCH clauses
        for pattern in self._match_patterns:
            if query:
                query += " "
            query += "MATCH "
            query += pattern

        # WHERE clauses
        if self._where_clauses:
            query += " WHERE "
            query += " AND ".join(self._where_clauses)

        # WITH clauses
        for clause in self._with_clauses:
            query += " WITH "
            query += clause

        # RETURN clause
        if self._return_clause:
            query += " RETURN "
            query += self._return_clause

        # ORDER BY clause
        if self._order_by:
            query += " ORDER BY "
            query += self._order_by

        # SKIP clause
        if self._skip is not None:
            query += " SKIP "
            query += str(self._skip)

        # LIMIT clause
        if self._limit is not None:
            query += " LIMIT "
            query += str(self._limit)

        return query.strip()

    def execute(self):
        '''
        Execute the query and return results

        Returns:
            QueryResult with rows and metadata

        Raises:
            QueryError: If query execution fails
        '''
        query = self.build()
        return self._session.query(query)


__all__ = ['QueryBuilder']