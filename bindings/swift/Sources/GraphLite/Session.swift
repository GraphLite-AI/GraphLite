import Foundation
import CGraphLite

/// A session for executing queries against a GraphLite database
///
/// Sessions maintain context for query execution, including the current schema and graph.
/// Always close sessions when done, or rely on automatic cleanup via `deinit`.
///
/// Example:
/// ```swift
/// let session = try db.createSession(username: "admin")
/// let result = try session.execute("MATCH (n) RETURN n")
/// session.close()
/// ```
public final class Session {
    private weak var database: GraphLite?
    private let sessionId: String
    private var isClosed = false

    internal init(database: GraphLite, sessionId: String) {
        self.database = database
        self.sessionId = sessionId
    }

    deinit {
        if !isClosed {
            close()
        }
    }

    /// Execute a GQL query
    ///
    /// - Parameter query: GQL query string
    /// - Returns: Query results
    /// - Throws: `GraphLiteError` if query execution fails
    public func execute(_ query: String) throws -> QueryResult {
        guard !isClosed else {
            throw GraphLiteError.databaseClosed
        }

        guard let db = database, let handle = db.databaseHandle else {
            throw GraphLiteError.databaseClosed
        }

        var error = GraphLiteErrorCode(rawValue: 0)
        guard let resultJsonPtr = graphlite_query(handle, sessionId, query, &error) else {
            throw GraphLiteError.from(error.rawValue)
        }

        let jsonString = String(cString: resultJsonPtr)
        graphlite_free_string(resultJsonPtr)

        // Parse JSON result
        guard let jsonData = jsonString.data(using: String.Encoding.utf8) else {
            throw GraphLiteError.invalidUtf8
        }

        let decoder = JSONDecoder()
        do {
            return try decoder.decode(QueryResult.self, from: jsonData)
        } catch {
            throw GraphLiteError.jsonError
        }
    }

    /// Close the session and release resources
    public func close() {
        guard !isClosed, let db = database, let handle = db.databaseHandle else {
            return
        }

        _ = graphlite_close_session(handle, sessionId, nil)
        isClosed = true
    }

    /// Check if session is closed
    public var closed: Bool {
        return isClosed
    }
}
