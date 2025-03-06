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

    let mut components = alloc::vec::Vec::new();
    let is_absolute = path.starts_with('/');

    // Process path components
    for component in path.split('/') {
        match component {
            "" | "." => continue,
            ".." => {
                if !components.is_empty() && components.last() != Some(&"..") {
                    components.pop();
                } else if !is_absolute {
                    components.push("..");
                }
            }
            _ => components.push(component),
        }
    }

    // Build normalized path
    let normalized = if is_absolute {
        let mut path = String::with_capacity(components.join("/").len() + 1);
        path.push('/');
        path.push_str(&components.join("/"));
        path
    } else if components.is_empty() {
        String::from(".")
    } else {
        components.join("/")
    };

    Some(normalized)
}

/// Get parent directory of path
#[allow(dead_code)]
pub fn get_parent_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return String::from("/");
    }
    let parent = parts[..parts.len() - 1].join("/");
    if parent.is_empty() {
        String::from("/")
    } else {
        format!("/{}", parent)
    }
}

/// Join two paths
/// If path is absolute path then return path directly
#[allow(dead_code)]
pub fn join_path(base: &str, path: &str) -> Option<String> {
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

    // Check illegal characters
    if path.contains("//") || path.contains('\0') {
        return false;
    }

    // Check path components
    let components: Vec<&str> = path.split('/').collect();

    // Handle special case for absolute path
    if path.starts_with('/') {
        // First component should be empty
        if !components[0].is_empty() {
            return false;
        }
        // Check from second component
        for component in &components[1..] {
            if component == &"." || component == &".." {
                continue;
            }
            if component.is_empty() && path != "/" {
                return false;
            }
        }
    } else {
        // Handle relative path
        for component in components {
            if component == "." || component == ".." {
                continue;
            }
            if component.is_empty() && path != "/" {
                return false;
            }
        }
    }

    true
}

/// Get filename part of path
#[allow(dead_code)]
pub fn get_basename(path: &str) -> Option<String> {
    split_path(path).map(|(_, name)| name.to_string())
}
