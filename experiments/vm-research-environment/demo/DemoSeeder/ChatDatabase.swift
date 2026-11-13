import Foundation
import SQLite3

/// A minimal SQLite connection to Messages' chat.db. The database's triggers call
/// `verify_chat()`, which Messages registers in its own connections; a no-op stand-in is
/// registered here so inserts into `chat` run.
final class ChatDatabase {
    private var handle: OpaquePointer?

    init(path: String) throws {
        guard sqlite3_open_v2(path, &handle, SQLITE_OPEN_READWRITE, nil) == SQLITE_OK else {
            throw SeederError.sqlite("open \(path): \(String(cString: sqlite3_errmsg(handle)))")
        }
        sqlite3_create_function_v2(handle, "verify_chat", 1, SQLITE_UTF8, nil, { context, _, _ in
            sqlite3_result_null(context)
        }, nil, nil, nil)
    }

    deinit { sqlite3_close_v2(handle) }

    func execute(_ sql: String) throws {
        guard sqlite3_exec(handle, sql, nil, nil, nil) == SQLITE_OK else { throw failure(sql) }
    }

    func count(_ table: String) throws -> Int {
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(handle, "SELECT count(*) FROM \(table)", -1, &statement, nil) == SQLITE_OK else {
            throw failure(table)
        }
        defer { sqlite3_finalize(statement) }
        guard sqlite3_step(statement) == SQLITE_ROW else { throw failure(table) }
        return Int(sqlite3_column_int64(statement, 0))
    }

    /// Inserts one row and returns its ROWID. Values are String, Int, Int64, Data or nil.
    func insert(_ table: String, _ values: [String: Any?]) throws -> Int64 {
        let columns = values.keys.sorted()
        let sql = "INSERT INTO \(table) (\(columns.joined(separator: ", "))) VALUES (\(columns.map { _ in "?" }.joined(separator: ", ")))"
        var statement: OpaquePointer?
        guard sqlite3_prepare_v2(handle, sql, -1, &statement, nil) == SQLITE_OK else { throw failure(sql) }
        defer { sqlite3_finalize(statement) }
        for (offset, column) in columns.enumerated() {
            let index = Int32(offset + 1)
            switch values[column] ?? nil {
            case nil: sqlite3_bind_null(statement, index)
            case let value as String: sqlite3_bind_text(statement, index, value, -1, transient)
            case let value as Int: sqlite3_bind_int64(statement, index, Int64(value))
            case let value as Int64: sqlite3_bind_int64(statement, index, value)
            case let value as Data:
                _ = value.withUnsafeBytes { sqlite3_bind_blob(statement, index, $0.baseAddress, Int32(value.count), transient) }
            case let value?: throw SeederError.invalid("unsupported value \(value) for \(table).\(column)")
            }
        }
        guard sqlite3_step(statement) == SQLITE_DONE else { throw failure(sql) }
        return sqlite3_last_insert_rowid(handle)
    }

    private func failure(_ context: String) -> SeederError {
        .sqlite("\(context): \(String(cString: sqlite3_errmsg(handle)))")
    }
}

private let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)
