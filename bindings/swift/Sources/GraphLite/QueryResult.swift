import Foundation

/// Result of a GQL query execution
///
/// Contains the column names (variables), rows of data, and row count.
public struct QueryResult: Codable {
    /// Column names returned by the query
    public let variables: [String]

    /// Raw rows from GraphLite (internal structure)
    private let rawRows: [RawRow]

    /// Execution time in milliseconds
    public let executionTimeMs: Int?

    /// Number of rows affected
    public let rowsAffected: Int?

    /// Computed rows: simplified view mapping column names to values
    public var rows: [[String: QueryValue]] {
        return rawRows.map { $0.values }
    }

    /// Total number of rows returned
    public var rowCount: Int {
        return rawRows.count
    }

    enum CodingKeys: String, CodingKey {
        case variables
        case rawRows = "rows"
        case executionTimeMs = "execution_time_ms"
        case rowsAffected = "rows_affected"
    }

    /// Internal structure for GraphLite row format
    struct RawRow: Codable {
        let values: [String: QueryValue]

        enum CodingKeys: String, CodingKey {
            case values
        }
    }
}

/// A value that can appear in query results
///
/// GraphLite values can be strings, integers, doubles, booleans, or null.
public enum QueryValue: Codable, CustomStringConvertible {
    case string(String)
    case integer(Int64)
    case double(Double)
    case boolean(Bool)
    case null

    enum CodingKeys: String, CodingKey {
        case String
        case Number
        case Boolean
        case Null
    }

    public init(from decoder: Decoder) throws {
        // GraphLite returns values as tagged unions: {"String": "value"}, {"Number": 123}
        let container = try decoder.container(keyedBy: CodingKeys.self)

        if let str = try? container.decode(String.self, forKey: .String) {
            self = .string(str)
        } else if let num = try? container.decode(Double.self, forKey: .Number) {
            // Check if it's an integer
            if num.truncatingRemainder(dividingBy: 1) == 0 && num >= Double(Int64.min) && num <= Double(Int64.max) {
                self = .integer(Int64(num))
            } else {
                self = .double(num)
            }
        } else if let bool = try? container.decode(Bool.self, forKey: .Boolean) {
            self = .boolean(bool)
        } else if container.contains(.Null) {
            self = .null
        } else {
            // Fallback: try to decode as simple value
            let singleContainer = try decoder.singleValueContainer()
            if singleContainer.decodeNil() {
                self = .null
            } else if let str = try? singleContainer.decode(String.self) {
                self = .string(str)
            } else if let int = try? singleContainer.decode(Int64.self) {
                self = .integer(int)
            } else if let dbl = try? singleContainer.decode(Double.self) {
                self = .double(dbl)
            } else if let bool = try? singleContainer.decode(Bool.self) {
                self = .boolean(bool)
            } else {
                throw DecodingError.typeMismatch(
                    QueryValue.self,
                    DecodingError.Context(
                        codingPath: decoder.codingPath,
                        debugDescription: "Cannot decode QueryValue"
                    )
                )
            }
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let str): try container.encode(str)
        case .integer(let int): try container.encode(int)
        case .double(let dbl): try container.encode(dbl)
        case .boolean(let bool): try container.encode(bool)
        case .null: try container.encodeNil()
        }
    }

    public var description: String {
        switch self {
        case .string(let str): return "\"\(str)\""
        case .integer(let int): return "\(int)"
        case .double(let dbl): return "\(dbl)"
        case .boolean(let bool): return "\(bool)"
        case .null: return "null"
        }
    }

    /// Convert to Swift native type
    public var asAny: Any? {
        switch self {
        case .string(let str): return str
        case .integer(let int): return int
        case .double(let dbl): return dbl
        case .boolean(let bool): return bool
        case .null: return nil
        }
    }
}
