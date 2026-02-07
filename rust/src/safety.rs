//! Path depth safety guard.
//!
//! Prevents operations on directories that are too shallow (e.g., drive root or
//! first-level directories) to avoid accidentally deleting files from other torrents.

use std::path::Path;

/// Check that the given path has at least `min_depth` components.
///
/// For Windows paths like `E:\Online\MyTorrent`, the components are:
///   1. `E:\`  (prefix + root)
///   2. `Online`
///   3. `MyTorrent`
///
/// We require at least 3 components (drive + 2 dirs) so that we only operate
/// inside subdirectories, never at the root or first-level.
pub fn check_depth(path: &Path, min_depth: usize) -> bool {
    // Canonicalize to resolve `.`, `..`, and get consistent component count
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => {
            // If path doesn't exist yet (e.g. unlock before download), try to count raw components
            return count_components(path) >= min_depth;
        }
    };
    count_components(&canonical) >= min_depth
}

/// Count path components. On Windows, the prefix (e.g., `E:`) and
/// root separator (`\`) together count as 1 component.
fn count_components(path: &Path) -> usize {
    let mut count = 0;
    for component in path.components() {
        match component {
            std::path::Component::Prefix(_) => {
                count += 1; // prefix counts as 1 (will merge with RootDir)
            }
            std::path::Component::RootDir => {
                // Already counted with Prefix on Windows
            }
            std::path::Component::Normal(_) => {
                count += 1;
            }
            _ => {}
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_depth() {
        // Too shallow
        assert!(!check_depth(Path::new("E:\\"), 3));
        assert!(!check_depth(Path::new("E:\\Online"), 3));

        // Deep enough
        assert!(check_depth(Path::new("E:\\Online\\MyTorrent"), 3));
        assert!(check_depth(Path::new("E:\\Online\\Category\\MyTorrent"), 3));
    }
}
