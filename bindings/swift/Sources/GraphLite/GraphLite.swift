import Foundation
import CGraphLite

/// GraphLite database instance
///
/// This class represents a connection to a GraphLite database.
/// Use it to create sessions for executing queries.
///
/// Example:
/// ```swift
/// let db = try GraphLite(path: "./mydb")
/// let session = try db.createSession(username: "admin")
/// ```
public final class GraphLite {
    private var handle: OpaquePointer?
    private let path: String

    /// Initialize a new GraphLite database connection
    ///
    /// - Parameter path: File system path to the database directory
    /// - Throws: `GraphLiteError` if database cannot be opened
    public init(path: String) throws {
        self.path = path

        var error = GraphLiteErrorCode(rawValue: 0)
        handle = graphlite_open(path, &error)

        guard handle != nil else {
            throw GraphLiteError.from(error.rawValue)
        }
    }

    deinit {
        if let handle = handle {
            graphlite_close(handle)
            self.handle = nil
        }
    }

    /// Create a new session for executing queries
    ///
    /// - Parameter username: Username for the session
    /// - Returns: A new `Session` instance
    /// - Throws: `GraphLiteError` if session creation fails
    public func createSession(username: String) throws -> Session {
        guard let handle = handle else {
            throw GraphLiteError.databaseClosed
        }

        var error = GraphLiteErrorCode(rawValue: 0)
        guard let sessionIdPtr = graphlite_create_session(handle, username, &error) else {
            throw GraphLiteError.from(error.rawValue)
        }

        let sessionId = String(cString: sessionIdPtr)
        graphlite_free_string(sessionIdPtr)

        return Session(database: self, sessionId: sessionId)
    }

    /// Get the database path
    public var databasePath: String {
        return path
    }

    /// Get the GraphLite version
    public static var version: String {
        let versionPtr = graphlite_version()
        return String(cString: versionPtr!)
    }

    // Internal access to handle for Session
    internal var databaseHandle: OpaquePointer? {
        return handle
    }
}
