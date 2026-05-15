use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Helper function to normalize a path by resolving `.` and `..` components.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                let is_empty = normalized.as_os_str().is_empty();
                let last_is_parent_dir =
                    normalized.components().next_back() == Some(Component::ParentDir);
                let last_is_root_dir =
                    normalized.components().next_back() == Some(Component::RootDir);

                if last_is_root_dir {
                    // Cannot go above root dir, just ignore ..
                } else if is_empty || last_is_parent_dir {
                    // We must preserve the .. for relative paths
                    normalized.push(Component::ParentDir);
                } else {
                    normalized.pop();
                }
            }
            Component::CurDir => {}
            _ => normalized.push(component),
        }
    }
    normalized
}

fn absolutize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Resolve symlinks in the existing prefix of a path while preserving missing suffixes.
fn resolve_path_for_comparison(path: &Path) -> PathBuf {
    let normalized = normalize_path(&absolutize_path(path));
    let mut suffix = PathBuf::new();
    let mut current = normalized.as_path();

    loop {
        match std::fs::canonicalize(current) {
            Ok(resolved) => {
                return if suffix.as_os_str().is_empty() {
                    resolved
                } else {
                    resolved.join(suffix)
                };
            }
            Err(_) => match current.parent() {
                Some(parent) => {
                    let Some(name) = current.file_name() else {
                        return normalized;
                    };
                    suffix = PathBuf::from(name).join(suffix);
                    current = parent;
                }
                None => return normalized,
            },
        }
    }
}

/// Permission set for sandbox execution
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PermissionSet {
    /// List of allowed file system paths
    pub allowed_paths: Vec<PathBuf>,
    /// Whether network access is permitted
    pub network_allowed: bool,
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

        let resolved_path = resolve_path_for_comparison(path);

        // Check if path is within any allowed directory
        for allowed in &self.allowed_paths {
            let resolved_allowed = resolve_path_for_comparison(allowed);
            if resolved_path.starts_with(&resolved_allowed) {
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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn test_path_traversal() {
        let perms = PermissionSet::new(vec![PathBuf::from("/tmp/sandbox")], false);

        // Standard within sandbox
        assert!(perms.check_path(Path::new("/tmp/sandbox/file.txt")));

        // Attempt to escape sandbox using ..
        assert!(!perms.check_path(Path::new("/tmp/sandbox/../../etc/passwd")));

        // Allowed path using .. that resolves within sandbox
        assert!(perms.check_path(Path::new("/tmp/sandbox/subdir/../file.txt")));

        let rel_perms = PermissionSet::new(vec![PathBuf::from("sandbox")], false);
        // Attempt to escape relative sandbox
        assert!(!rel_perms.check_path(Path::new("sandbox/../../etc/passwd")));
        assert!(!rel_perms.check_path(Path::new("../../etc/passwd")));
        assert!(rel_perms.check_path(Path::new("sandbox/subdir/../file.txt")));
    }

    #[test]
    fn test_allowed_path_is_normalized_before_comparison() {
        let perms = PermissionSet::new(vec![PathBuf::from("/tmp/sandbox/../sandbox")], false);

        assert!(perms.check_path(Path::new("/tmp/sandbox/file.txt")));
        assert!(!perms.check_path(Path::new("/tmp/other/file.txt")));
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_escape_is_denied() {
        use std::os::unix::fs::symlink;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "abot-sandbox-symlink-test-{}-{}",
            std::process::id(),
            unique
        ));
        let allowed = root.join("allowed");
        let outside = root.join("outside");
        let inside = allowed.join("inside");
        let escape = allowed.join("escape");

        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink(&outside, &escape).unwrap();

        let perms = PermissionSet::new(vec![allowed.clone()], false);

        assert!(perms.check_path(&inside.join("file.txt")));
        assert!(!perms.check_path(&escape.join("secret.txt")));
        assert!(!perms.check_path(&escape.join("new.txt")));

        fs::remove_dir_all(&root).unwrap();
    }
}
