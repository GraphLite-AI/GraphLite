import Foundation
import GraphLite

/// Observable database manager for SwiftUI
@MainActor
class DatabaseManager: ObservableObject {
    @Published var people: [Person] = []
    @Published var isLoading = false
    @Published var errorMessage: String?

    private var db: GraphLite?
    private var session: Session?
    private let dbPath: String

    init() {
        // Use app's document directory for the database
        let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        self.dbPath = documentsPath.appendingPathComponent("graphlite.db").path

        setupDatabase()
    }

    private func setupDatabase() {
        do {
            // Initialize database
            db = try GraphLite(path: dbPath)

            // Create session
            session = try db?.createSession(username: "ios_user")

            // Setup schema and graph
            try session?.execute("CREATE SCHEMA IF NOT EXISTS /app")
            try session?.execute("SESSION SET SCHEMA /app")
            try session?.execute("CREATE GRAPH IF NOT EXISTS /app/people")
            try session?.execute("SESSION SET GRAPH /app/people")

            print("✓ Database initialized at: \(dbPath)")

            // Load initial data
            Task {
                await loadPeople()
            }
        } catch {
            errorMessage = "Failed to setup database: \(error.localizedDescription)"
            print("❌ Database error: \(error)")
        }
    }

    func loadPeople() async {
        isLoading = true
        errorMessage = nil

        do {
            let result = try session?.execute("""
                MATCH (p:Person)
                RETURN p.name, p.age, p.city
                ORDER BY p.name
            """)

            guard let result = result else {
                throw GraphLiteError.sessionError
            }

            // Parse results into Person objects
            var loadedPeople: [Person] = []
            for row in result.rows {
                if case .string(let name) = row["p.name"],
                   case .integer(let age) = row["p.age"],
                   case .string(let city) = row["p.city"] {
                    loadedPeople.append(Person(
                        id: UUID(), // Note: in production, store actual ID
                        name: name,
                        age: Int(age),
                        city: city
                    ))
                }
            }

            people = loadedPeople
        } catch {
            errorMessage = "Failed to load people: \(error.localizedDescription)"
        }

        isLoading = false
    }

    func addPerson(name: String, age: Int, city: String) async {
        guard !name.isEmpty, !city.isEmpty else {
            errorMessage = "Name and city cannot be empty"
            return
        }

        isLoading = true
        errorMessage = nil

        do {
            _ = try session?.execute("""
                INSERT (:Person {name: '\(name)', age: \(age), city: '\(city)'})
            """)

            await loadPeople()
        } catch {
            errorMessage = "Failed to add person: \(error.localizedDescription)"
            isLoading = false
        }
    }

    func deletePerson(name: String) async {
        isLoading = true
        errorMessage = nil

        do {
            _ = try session?.execute("""
                MATCH (p:Person WHERE p.name = '\(name)')
                DELETE p
            """)

            await loadPeople()
        } catch {
            errorMessage = "Failed to delete person: \(error.localizedDescription)"
            isLoading = false
        }
    }

    func searchPeople(city: String) async {
        isLoading = true
        errorMessage = nil

        do {
            let result = try session?.execute("""
                MATCH (p:Person WHERE p.city = '\(city)')
                RETURN p.name, p.age, p.city
                ORDER BY p.name
            """)

            guard let result = result else {
                throw GraphLiteError.sessionError
            }

            var loadedPeople: [Person] = []
            for row in result.rows {
                if case .string(let name) = row["p.name"],
                   case .integer(let age) = row["p.age"],
                   case .string(let city) = row["p.city"] {
                    loadedPeople.append(Person(
                        id: UUID(),
                        name: name,
                        age: Int(age),
                        city: city
                    ))
                }
            }

            people = loadedPeople
        } catch {
            errorMessage = "Failed to search: \(error.localizedDescription)"
        }

        isLoading = false
    }

    func clearDatabase() async {
        isLoading = true
        errorMessage = nil

        do {
            _ = try session?.execute("MATCH (p:Person) DELETE p")
            await loadPeople()
        } catch {
            errorMessage = "Failed to clear database: \(error.localizedDescription)"
            isLoading = false
        }
    }

    deinit {
        session?.close()
    }
}

/// Simple Person model for the demo
struct Person: Identifiable {
    let id: UUID
    let name: String
    let age: Int
    let city: String
}
