/// Session management mode for QueryCoordinator
///
/// Determines whether sessions are managed at the instance level (embedded mode)
/// or at the process level (server/daemon mode).
///
/// # Modes
///
/// - **Instance**: Each QueryCoordinator instance has its own isolated session pool.
///   Sessions are not shared between instances. Use this mode for embedded databases
///   where each application instance needs complete isolation.
///
/// - **Global**: All QueryCoordinator instances share a process-wide session pool.
///   Sessions created in one coordinator are visible to all coordinators in the same
///   process. Use this mode for server/daemon applications where multiple coordinators
///   need to access the same user sessions.
///
/// # Examples
///
/// ```rust,no_run
/// use graphlite::coordinator::QueryCoordinator;
/// use graphlite::session::SessionMode;
///
/// // Embedded mode - each instance isolated
/// let coord1 = QueryCoordinator::from_path_with_mode("db1.graphlite", SessionMode::Instance)?;
/// let coord2 = QueryCoordinator::from_path_with_mode("db2.graphlite", SessionMode::Instance)?;
/// // coord1 and coord2 have separate session pools
///
/// // Server mode - shared session pool
/// let coord1 = QueryCoordinator::from_path_with_mode("db.graphlite", SessionMode::Global)?;
/// let coord2 = QueryCoordinator::from_path_with_mode("db.graphlite", SessionMode::Global)?;
/// // coord1 and coord2 share the same session pool
/// # Ok::<(), String>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum SessionMode {
    /// Instance-based session management (default)
    ///
    /// Each QueryCoordinator instance maintains its own isolated session pool.
    /// Sessions are not shared between instances.
    ///
    /// Use this mode when:
    /// - Running embedded database in application
    /// - Each database instance needs complete isolation
    /// - Testing with parallel test execution
    #[default]
    Instance,

    /// Process-wide session management
    ///
    /// All QueryCoordinator instances share a single global session pool.
    /// Sessions created in one coordinator are visible to all coordinators.
    ///
    /// Use this mode when:
    /// - Running GraphLite as a server/daemon
    /// - Multiple coordinators need to access same sessions
    /// - Implementing connection pooling
    Global,
}


impl SessionMode {
    /// Returns true if this is Instance mode
    pub fn is_instance(&self) -> bool {
        matches!(self, SessionMode::Instance)
    }

    /// Returns true if this is Global mode
    pub fn is_global(&self) -> bool {
        matches!(self, SessionMode::Global)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode() {
        assert_eq!(SessionMode::default(), SessionMode::Instance);
    }

    #[test]
    fn test_is_instance() {
        assert!(SessionMode::Instance.is_instance());
        assert!(!SessionMode::Global.is_instance());
    }

    #[test]
    fn test_is_global() {
        assert!(SessionMode::Global.is_global());
        assert!(!SessionMode::Instance.is_global());
    }

    #[test]
    fn test_clone() {
        let mode = SessionMode::Global;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_copy() {
        let mode = SessionMode::Instance;
        let copied = mode;
        assert_eq!(mode, copied);
    }
}
