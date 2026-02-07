//! Mode 1: Sync — delete extra files not in the torrent.
//!
//! Steps:
//! 1. Sleep 3 seconds (wait for uTorrent to release file handles)
//! 2. Validate path depth (safety guard)
//! 3. Parse .torrent → extract expected file list
//! 4. Walk directory depth-first (children before parents)
//! 5. Delete files not in the expected set
//! 6. Delete empty directories

use crate::bencode;
use crate::logger;
use crate::safety;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// Run the sync operation.
pub fn run(torrent_path: &str, dir_path: &str) {
    // Step 1: Delay 3 seconds
    thread::sleep(Duration::from_secs(3));

    let dir = Path::new(dir_path);

    // Step 2: Safety guard
    if !safety::check_depth(dir, 3) {
        logger::log(&format!(
            "SYNC {:?} — path too shallow, aborted",
            dir_path
        ));
        std::process::exit(1);
    }

    // Step 3: Parse torrent file
    let expected_files = match bencode::parse_torrent_file(Path::new(torrent_path)) {
        Ok(files) => files,
        Err(e) => {
            logger::log(&format!(
                "SYNC {:?} — {}",
                torrent_path, e
            ));
            std::process::exit(1);
        }
    };

    // Build HashSet of expected relative paths
    let expected: HashSet<PathBuf> = expected_files.into_iter().collect();

    if !dir.exists() {
        logger::log(&format!(
            "SYNC {:?} — directory does not exist, aborted",
            dir_path
        ));
        std::process::exit(1);
    }

    // Step 4-5: Walk and delete
    let mut deleted_files = 0u32;
    let mut deleted_dirs = 0u32;

    // Collect all entries depth-first (children before parents)
    let entries = walk_depth_first(dir);

    for entry_path in &entries {
        let relative = match entry_path.strip_prefix(dir) {
            Ok(r) => r.to_path_buf(),
            Err(_) => continue,
        };

        if entry_path.is_dir() {
            // Try to remove empty directory (non-recursive, safe)
            if fs::remove_dir(entry_path).is_ok() {
                deleted_dirs += 1;
            }
        } else if !expected.contains(&relative) {
            // File not in torrent — delete it
            match fs::remove_file(entry_path) {
                Ok(()) => deleted_files += 1,
                Err(e) => {
                    logger::log(&format!(
                        "SYNC {:?} — failed to delete {:?}: {}",
                        dir_path, relative, e
                    ));
                }
            }
        }
    }

    // Step 6: Log summary
    if deleted_files == 0 && deleted_dirs == 0 {
        logger::log(&format!(
            "SYNC {:?} — clean, nothing to remove",
            dir_path
        ));
    } else {
        logger::log(&format!(
            "SYNC {:?} — deleted {} files, {} empty dirs",
            dir_path, deleted_files, deleted_dirs
        ));
    }
}

/// Walk a directory tree depth-first, returning paths with children before parents.
/// This ensures we can delete files first, then their parent directories if empty.
fn walk_depth_first(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    walk_recursive(root, &mut result);
    result
}

/// Recursive helper: collect files first, then directories (post-order).
fn walk_recursive(dir: &Path, result: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    // Collect entries and sort for deterministic behavior
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else {
            files.push(path);
        }
    }

    // Recurse into subdirectories first (depth-first)
    for d in &dirs {
        walk_recursive(d, result);
    }

    // Add files
    for f in files {
        result.push(f);
    }

    // Add directories after their contents (post-order)
    for d in dirs {
        result.push(d);
    }
}
