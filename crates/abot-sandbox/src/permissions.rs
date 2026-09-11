use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Permission set for sandbox execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionSet {
    /// List of allowed file system paths
    pub allowed_paths: Vec<PathBuf>,
    /// Whether network access is permitted
    pub network_allowed: bool,
}

impl Default for PermissionSet {
    fn default() -> Self {
        Self {
            allowed_paths: vec![],
            network_allowed: false,
        }
    }
}

impl PermissionSet {
    /// Create a new permission set
    pub fn new(allowed_paths: Vec<PathBuf>, network_allowed: bool) -> Self {
        Self {
            allowed_paths,
            network_allowed,
        }
    }

    /// Check if a file path is allowed
    pub fn check_path(&self, path: &Path) -> bool {
        // Empty allowed_paths means deny all file access
        if self.allowed_paths.is_empty() {
            return false;
        }

        // Check if path is within any allowed directory
        for allowed in &self.allowed_paths {
            if path.starts_with(allowed) {
                return true;
            }
        }

        false
    }

    /// Check if network access is allowed
    pub fn check_network(&self) -> bool {
        self.network_allowed
    }

    /// Add an allowed path
    pub fn add_path(&mut self, path: PathBuf) {
        if !self.allowed_paths.contains(&path) {
            self.allowed_paths.push(path);
        }
    }

    /// Enable network access
    pub fn enable_network(&mut self) {
        self.network_allowed = true;
    }

    /// Disable network access
    pub fn disable_network(&mut self) {
        self.network_allowed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_check_allowed() {
        let perms = PermissionSet::new(vec![PathBuf::from("/tmp/sandbox")], false);

        assert!(perms.check_path(Path::new("/tmp/sandbox/file.txt")));
        assert!(!perms.check_path(Path::new("/etc/passwd")));
    }

    #[test]
    fn test_path_check_empty() {
        let perms = PermissionSet::default();
        assert!(!perms.check_path(Path::new("/tmp/file.txt")));
    }

    #[test]
    fn test_network_permission() {
        let mut perms = PermissionSet::default();
        assert!(!perms.check_network());

        perms.enable_network();
        assert!(perms.check_network());

        perms.disable_network();
        assert!(!perms.check_network());
    }

    #[test]
    fn test_add_path() {
        let mut perms = PermissionSet::default();
        perms.add_path(PathBuf::from("/tmp/work"));

        assert!(perms.check_path(Path::new("/tmp/work/output.txt")));
    }

    #[test]
    fn test_multiple_allowed_paths() {
        let perms = PermissionSet::new(
            vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")],
            false,
        );

        assert!(perms.check_path(Path::new("/tmp/a/file.txt")));
        assert!(perms.check_path(Path::new("/tmp/b/file.txt")));
        assert!(!perms.check_path(Path::new("/tmp/c/file.txt")));
    }
}
