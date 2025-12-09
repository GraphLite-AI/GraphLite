package com.deepgraph.graphlite;

import com.sun.jna.*;
import com.sun.jna.ptr.IntByReference;
import org.json.JSONObject;
import org.json.JSONArray;

import java.io.Closeable;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.*;

/**
 * GraphLite Java API
 *
 * High-level Java wrapper around GraphLite C FFI using JNA.
 *
 * Example:
 * <pre>{@code
 * try (GraphLite db = GraphLite.open("./mydb")) {
 *     String session = db.createSession("admin");
 *     QueryResult result = db.query(session, "MATCH (n:Person) RETURN n");
 *     for (Map<String, Object> row : result.getRows()) {
 *         System.out.println(row);
 *     }
 * }
 * }</pre>
 */
public class GraphLite implements Closeable {

    /**
     * Error codes from GraphLite FFI
     */
    public enum ErrorCode {
        SUCCESS(0),
        NULL_POINTER(1),
        INVALID_UTF8(2),
        DATABASE_OPEN_ERROR(3),
        SESSION_ERROR(4),
        QUERY_ERROR(5),
        PANIC_ERROR(6),
        JSON_ERROR(7);

        private final int code;

        ErrorCode(int code) {
            this.code = code;
        }

        public int getCode() {
            return code;
        }

        public static ErrorCode fromInt(int code) {
            for (ErrorCode ec : values()) {
                if (ec.code == code) {
                    return ec;
                }
            }
            return null;
        }
    }

    /**
     * GraphLite exception
     */
    public static class GraphLiteException extends RuntimeException {
        private final ErrorCode errorCode;

        public GraphLiteException(ErrorCode errorCode, String message) {
            super(String.format("GraphLite error (%s): %s", errorCode, message));
            this.errorCode = errorCode;
        }

        public ErrorCode getErrorCode() {
            return errorCode;
        }
    }

    // JNA interface to GraphLite C library
    private interface GraphLiteNative extends Library {
        GraphLiteNative INSTANCE = loadLibrary();

        Pointer graphlite_open(String path, IntByReference error);
        Pointer graphlite_create_session(Pointer db, String username, IntByReference error);
        Pointer graphlite_query(Pointer db, String sessionId, String query, IntByReference error);
        int graphlite_close_session(Pointer db, String sessionId, IntByReference error);
        void graphlite_free_string(Pointer str);
        void graphlite_close(Pointer db);
        Pointer graphlite_version();

        static GraphLiteNative loadLibrary() {
            String libName;
            String os = System.getProperty("os.name").toLowerCase();

            if (os.contains("mac")) {
                libName = "graphlite_ffi";
            } else if (os.contains("win")) {
                libName = "graphlite_ffi";
            } else {
                libName = "graphlite_ffi";
            }

            // Try to find library in common locations
            String[] searchPaths = {
                "target/release",
                "target/debug",
                "/usr/local/lib",
                "/usr/lib",
                "."
            };

            for (String path : searchPaths) {
                try {
                    NativeLibrary.addSearchPath(libName, path);
                } catch (Exception e) {
                    // Continue searching
                }
            }

            try {
                return Native.load(libName, GraphLiteNative.class);
            } catch (UnsatisfiedLinkError e) {
                throw new RuntimeException(
                    "Could not load GraphLite library. " +
                    "Please build the FFI library first: cargo build --release -p graphlite-ffi",
                    e
                );
            }
        }
    }

    private Pointer dbHandle;
    private final Set<String> sessions = new HashSet<>();

    private GraphLite(Pointer dbHandle) {
        this.dbHandle = dbHandle;
    }

    /**
     * Open a GraphLite database
     *
     * @param path Path to database directory
     * @return GraphLite instance
     * @throws GraphLiteException if database cannot be opened
     */
    public static GraphLite open(String path) {
        IntByReference error = new IntByReference(0);
        Pointer dbHandle = GraphLiteNative.INSTANCE.graphlite_open(path, error);

        if (dbHandle == null) {
            ErrorCode errorCode = ErrorCode.fromInt(error.getValue());
            throw new GraphLiteException(errorCode, "Failed to open database at " + path);
        }

        return new GraphLite(dbHandle);
    }

    /**
     * Create a new session for the given user
     *
     * @param username Username for the session
     * @return Session ID string
     * @throws GraphLiteException if session creation fails
     */
    public String createSession(String username) {
        checkClosed();

        IntByReference error = new IntByReference(0);
        Pointer sessionPtr = GraphLiteNative.INSTANCE.graphlite_create_session(
            dbHandle, username, error
        );

        if (sessionPtr == null) {
            ErrorCode errorCode = ErrorCode.fromInt(error.getValue());
            throw new GraphLiteException(errorCode, "Failed to create session for user '" + username + "'");
        }

        String sessionId = sessionPtr.getString(0);
        GraphLiteNative.INSTANCE.graphlite_free_string(sessionPtr);
        sessions.add(sessionId);

        return sessionId;
    }

    /**
     * Execute a GQL query
     *
     * @param sessionId Session ID from createSession()
     * @param query GQL query string
     * @return QueryResult with rows and metadata
     * @throws GraphLiteException if query execution fails
     */
    public QueryResult query(String sessionId, String query) {
        checkClosed();

        IntByReference error = new IntByReference(0);
        Pointer resultPtr = GraphLiteNative.INSTANCE.graphlite_query(
            dbHandle, sessionId, query, error
        );

        if (resultPtr == null) {
            ErrorCode errorCode = ErrorCode.fromInt(error.getValue());
            throw new GraphLiteException(errorCode, "Query failed: " + query.substring(0, Math.min(100, query.length())));
        }

        try {
            String resultJson = resultPtr.getString(0);
            return new QueryResult(resultJson);
        } finally {
            GraphLiteNative.INSTANCE.graphlite_free_string(resultPtr);
        }
    }

    /**
     * Execute a statement without returning results
     *
     * @param sessionId Session ID from createSession()
     * @param statement GQL statement to execute
     * @throws GraphLiteException if execution fails
     */
    public void execute(String sessionId, String statement) {
        query(sessionId, statement);
    }

    /**
     * Close a session
     *
     * @param sessionId Session ID to close
     * @throws GraphLiteException if session close fails
     */
    public void closeSession(String sessionId) {
        if (dbHandle == null) {
            return;
        }

        IntByReference error = new IntByReference(0);
        int result = GraphLiteNative.INSTANCE.graphlite_close_session(dbHandle, sessionId, error);

        if (result != 0) {
            ErrorCode errorCode = ErrorCode.fromInt(error.getValue());
            throw new GraphLiteException(errorCode, "Failed to close session " + sessionId);
        }

        sessions.remove(sessionId);
    }

    /**
     * Close the database and all sessions
     */
    @Override
    public void close() {
        if (dbHandle != null) {
            // Close all open sessions
            for (String sessionId : new ArrayList<>(sessions)) {
                try {
                    closeSession(sessionId);
                } catch (GraphLiteException e) {
                    // Ignore errors during cleanup
                }
            }

            GraphLiteNative.INSTANCE.graphlite_close(dbHandle);
            dbHandle = null;
        }
    }

    /**
     * Get GraphLite version
     *
     * @return Version string
     */
    public static String version() {
        Pointer versionPtr = GraphLiteNative.INSTANCE.graphlite_version();
        if (versionPtr != null) {
            // Note: version returns a static string, do NOT free it
            return versionPtr.getString(0);
        }
        return "unknown";
    }

    private void checkClosed() {
        if (dbHandle == null) {
            throw new GraphLiteException(ErrorCode.NULL_POINTER, "Database is closed");
        }
    }
}
