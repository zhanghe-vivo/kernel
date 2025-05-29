//! Provides utility functions for path processing
#![allow(dead_code)]

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

/// Split path into parent directory and filename
/// Returns (parent_path, file_name) or None
pub fn split_path(path: &str) -> Option<(&str, &str)> {
    if path.is_empty() {
        return None;
    }

    if path == "/" {
        return Some(("/", ""));
    }

    match path.rsplit_once('/') {
        Some((parent, name)) => {
            if parent.is_empty() {
                Some(("/", name)) // Special handling for root directory
            } else {
                Some((parent, name))
            }
        }
        None => Some((".", path)), // Relative path
    }
}

/// Normalize path by removing "." and ".."
/// Returns normalized path or None
pub fn normalize_path(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }

    let is_absolute = path.starts_with('/');
    let mut result = String::with_capacity(path.len());
    let mut skip_count = 0;

    // Process path components in reverse order to handle ".." correctly
    for component in path.split('/').rev() {
        match component {
            "" | "." => continue,
            ".." => {
                skip_count += 1;
            }
            _ => {
                if skip_count > 0 {
                    skip_count -= 1;
                } else {
                    if !result.is_empty() {
                        result.insert(0, '/');
                    }
                    result.insert_str(0, component);
                }
            }
        }
    }

    // Handle remaining ".." for relative paths
    if !is_absolute {
        for _ in 0..skip_count {
            if !result.is_empty() {
                result.insert(0, '/');
            }
            result.insert_str(0, "..");
        }
    }

    // Handle empty result
    if result.is_empty() {
        result = if is_absolute {
            String::from("/")
        } else {
            String::from(".")
        };
    } else if is_absolute {
        result.insert(0, '/');
    }

    Some(result)
}

/// Join two paths
/// If path is absolute path then return path directly
#[allow(dead_code)]
pub fn join_path(base: &str, path: &str) -> Option<String> {
    if base.is_empty() {
        return normalize_path(path);
    }

    if path.starts_with('/') {
        return normalize_path(path);
    }

    let mut joined = String::from(base);
    if !base.ends_with('/') {
        joined.push('/');
    }
    joined.push_str(path);

    normalize_path(&joined)
}

/// Check if the path is valid
pub fn is_valid_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    if path == "/" {
        return true;
    }

    let mut iter = path.split('/');

    // Handle absolute path
    if path.starts_with('/') {
        // First component should be empty for absolute path
        if !iter.next().unwrap_or("").is_empty() {
            return false;
        }
    }

    for component in iter {
        if component.is_empty() || component.contains('\0') {
            return false;
        }
    }

    true
}

/// Get filename part of path
#[allow(dead_code)]
pub fn get_basename(path: &str) -> Option<String> {
    split_path(path).map(|(_, name)| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bluekernel_test_macro::test;

    #[test]
    fn test_is_valid_path() {
        // Valid paths
        assert!(is_valid_path("/"));
        assert!(is_valid_path("/usr"));
        assert!(is_valid_path("/usr/bin"));
        assert!(is_valid_path("usr/bin"));
        assert!(is_valid_path("./usr/bin"));
        assert!(is_valid_path("../usr/bin"));

        // Invalid paths
        assert!(!is_valid_path(""));
        assert!(!is_valid_path("//"));
        assert!(!is_valid_path("/usr//bin"));
        assert!(!is_valid_path("/usr/\0bin"));
        assert!(!is_valid_path("usr//bin"));
    }

    #[test]
    fn test_normalize_path() {
        // Absolute paths
        assert_eq!(normalize_path("/"), Some("/".to_string()));
        assert_eq!(normalize_path("/usr/bin"), Some("/usr/bin".to_string()));
        assert_eq!(normalize_path("/usr/./bin"), Some("/usr/bin".to_string()));
        assert_eq!(normalize_path("/usr/../bin"), Some("/bin".to_string()));
        assert_eq!(normalize_path("/usr/../../bin"), Some("/bin".to_string()));

        // Relative paths
        assert_eq!(normalize_path("usr/bin"), Some("usr/bin".to_string()));
        assert_eq!(normalize_path("./usr/bin"), Some("usr/bin".to_string()));
        assert_eq!(normalize_path("../usr/bin"), Some("../usr/bin".to_string()));
        assert_eq!(normalize_path("usr/./bin"), Some("usr/bin".to_string()));
        assert_eq!(normalize_path("usr/../bin"), Some("bin".to_string()));

        // Edge cases
        assert_eq!(normalize_path(""), None);
        assert_eq!(normalize_path("."), Some(".".to_string()));
        assert_eq!(normalize_path(".."), Some("..".to_string()));
    }

    #[test]
    fn test_split_path() {
        // Absolute paths
        assert_eq!(split_path("/usr/bin"), Some(("/usr", "bin")));
        assert_eq!(split_path("/usr"), Some(("/", "usr")));
        assert_eq!(split_path("/"), Some(("/", "")));

        // Relative paths
        assert_eq!(split_path("usr/bin"), Some(("usr", "bin")));
        assert_eq!(split_path("bin"), Some((".", "bin")));

        // Edge cases
        assert_eq!(split_path(""), None);
    }

    #[test]
    fn test_join_path() {
        // Absolute paths
        assert_eq!(join_path("/usr", "/bin"), Some("/bin".to_string()));
        assert_eq!(join_path("/usr", "bin"), Some("/usr/bin".to_string()));
        assert_eq!(join_path("/usr/", "bin"), Some("/usr/bin".to_string()));

        // Relative paths
        assert_eq!(join_path("usr", "bin"), Some("usr/bin".to_string()));
        assert_eq!(join_path("usr/", "bin"), Some("usr/bin".to_string()));
        assert_eq!(join_path(".", "bin"), Some("bin".to_string()));
        assert_eq!(join_path("..", "bin"), Some("../bin".to_string()));

        // Edge cases
        assert_eq!(join_path("", "bin"), Some("bin".to_string()));
    }

    #[test]
    fn test_get_basename() {
        // Absolute paths
        assert_eq!(get_basename("/usr/bin"), Some("bin".to_string()));
        assert_eq!(get_basename("/usr"), Some("usr".to_string()));
        assert_eq!(get_basename("/"), Some("".to_string()));

        // Relative paths
        assert_eq!(get_basename("usr/bin"), Some("bin".to_string()));
        assert_eq!(get_basename("bin"), Some("bin".to_string()));

        // Edge cases
        assert_eq!(get_basename(""), None);
    }
}
