import Foundation

/// Errors that can occur when using GraphLite
public enum GraphLiteError: Error, LocalizedError {
    case databaseClosed
    case nullPointer
    case invalidUtf8
    case databaseOpenError
    case sessionError
    case queryError
    case panicError
    case jsonError
    case unknown(code: UInt32)

    public var errorDescription: String? {
        switch self {
        case .databaseClosed:
            return "Database connection is closed"
        case .nullPointer:
            return "Null pointer error in C FFI"
        case .invalidUtf8:
            return "Invalid UTF-8 string passed to C FFI"
        case .databaseOpenError:
            return "Failed to open database"
        case .sessionError:
            return "Session management error"
        case .queryError:
            return "Query execution failed"
        case .panicError:
            return "Internal panic occurred in Rust code"
        case .jsonError:
            return "JSON serialization/deserialization failed"
        case .unknown(let code):
            return "Unknown error (code: \(code))"
        }
    }

    static func from(_ code: UInt32) -> GraphLiteError {
        switch code {
        case 0: fatalError("Success is not an error")
        case 1: return .nullPointer
        case 2: return .invalidUtf8
        case 3: return .databaseOpenError
        case 4: return .sessionError
        case 5: return .queryError
        case 6: return .panicError
        case 7: return .jsonError
        default: return .unknown(code: code)
        }
    }
}
