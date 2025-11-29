"""
Error types for the GraphLite SDK
"""

from typing import Union
import json

class GraphLiteError(Exception):
    """
    Base exception for all GraphLite SDK errors.
    """
    def __init__(self, message: str):
        self.message = message
        super().__init__(message)

class GraphLiteCoreError(GraphLiteError):
    """Error from the core GraphLite library"""
    def __init__(self, message: str):
        super().__init__(f"GraphLite error: {message}")

class SessionError(GraphLiteError):
    """Session-related errors"""
    def __init__(self, message: str):
        super().__init__(f"Session error: {message}")

class QueryError(GraphLiteError):
    """Query execution errors"""
    def __init__(self, message: str):
        super().__init__(f"Query error: {message}")

class TransactionError(GraphLiteError):
    """Transaction errors"""
    def __init__(self, message: str):
        super().__init__(f"Transaction error: {message}")

class SerializationError(GraphLiteError):
    """Serialization/deserialization errors"""
    def __init__(self, message: str):
        super().__init__(f"Serialization error: {message}")
    
    @classmethod
    def from_json_error(cls, error: json.JSONDecodeError) -> "SerializationError":
        """Create SerializationError from json.JSONDecodeError"""
        return cls(f"JSON error: {error.msg} at line {error.lineno}, column {error.colno}")

class TypeConversionError(GraphLiteError):
    """Type conversion errors"""
    def __init__(self, message: str):
        super().__init__(f"Type conversion error: {message}")

class InvalidOperationError(GraphLiteError):
    """Invalid operation errors"""
    def __init__(self, message: str):
        super().__init__(f"Invalid operation: {message}")

class NotFoundError(GraphLiteError):
    """Resource not found errors"""
    def __init__(self, message: str):
        super().__init__(f"Not found: {message}")

class ConnectionError(GraphLiteError):
    """Connection errors"""
    def __init__(self, message: str):
        super().__init__(f"Connection error: {message}")

class IoError(GraphLiteError):
    """I/O errors"""
    def __init__(self, message: str):
        super().__init__(f"I/O error: {message}")
    
    @classmethod
    def from_io_error(cls, error: OSError) -> "IoError":
        """Create IoError from Python OSError (IOError in Python 3)"""
        filename = getattr(error, 'filename', None) or ''
        strerror = getattr(error, 'strerror', None) or str(error)
        return cls(f"{strerror}: {filename}" if filename else strerror)


# ============================================================================
# Error Conversion Helpers
# ============================================================================

def from_string(message: Union[str, bytes]) -> GraphLiteCoreError:
    """
    Convert a string to GraphLiteCoreError.
    """
    if isinstance(message, bytes):
        message = message.decode('utf-8', errors='replace')
    return GraphLiteCoreError(message)


def from_json_error(error: json.JSONDecodeError) -> SerializationError:
    """
    Convert json.JSONDecodeError to SerializationError.
    """
    return SerializationError.from_json_error(error)


def from_io_error(error: OSError) -> IoError:
    """
    Convert OSError to IoError.
    """
    return IoError.from_io_error(error)
