import XCTest
@testable import GraphLite

final class GraphLiteTests: XCTestCase {
    var tempDir: URL!

    override func setUp() async throws {
        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() async throws {
        if let tempDir = tempDir {
            try? FileManager.default.removeItem(at: tempDir)
        }
    }

    func testDatabaseCreation() throws {
        let dbPath = tempDir.appendingPathComponent("test.db").path
        let db = try GraphLite(path: dbPath)
        XCTAssertNotNil(db)
    }

    func testSessionCreation() throws {
        let dbPath = tempDir.appendingPathComponent("test.db").path
        let db = try GraphLite(path: dbPath)
        let session = try db.createSession(username: "test_user")
        XCTAssertNotNil(session)
        XCTAssertFalse(session.closed)
    }

    func testBasicQuery() throws {
        let dbPath = tempDir.appendingPathComponent("test.db").path
        let db = try GraphLite(path: dbPath)
        let session = try db.createSession(username: "admin")

        // Create schema and graph
        _ = try session.execute("CREATE SCHEMA /test")
        _ = try session.execute("SESSION SET SCHEMA /test")
        _ = try session.execute("CREATE GRAPH /test/demo")
        _ = try session.execute("SESSION SET GRAPH /test/demo")

        // Insert data
        _ = try session.execute("INSERT (:Person {name: 'Alice', age: 30})")

        // Query data
        let result = try session.execute("MATCH (p:Person) RETURN p.name, p.age")

        XCTAssertEqual(result.rowCount, 1)
        XCTAssertEqual(result.variables.count, 2)
        XCTAssertTrue(result.variables.contains("p.name"))
        XCTAssertTrue(result.variables.contains("p.age"))

        let row = result.rows[0]
        if case .string(let name) = row["p.name"] {
            XCTAssertEqual(name, "Alice")
        } else {
            XCTFail("Expected string value for p.name")
        }

        if case .integer(let age) = row["p.age"] {
            XCTAssertEqual(age, 30)
        } else {
            XCTFail("Expected integer value for p.age")
        }
    }

    func testSessionClose() throws {
        let dbPath = tempDir.appendingPathComponent("test.db").path
        let db = try GraphLite(path: dbPath)
        let session = try db.createSession(username: "test_user")

        XCTAssertFalse(session.closed)
        session.close()
        XCTAssertTrue(session.closed)

        // Should throw error on closed session
        XCTAssertThrowsError(try session.execute("CREATE SCHEMA /test"))
    }

    func testVersion() {
        let version = GraphLite.version
        XCTAssertFalse(version.isEmpty)
        print("GraphLite version: \(version)")
    }
}
