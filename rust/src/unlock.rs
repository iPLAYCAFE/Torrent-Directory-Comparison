//! Mode 2: Unlock — kill processes locking files in a directory.
//!
//! Uses Win32 Restart Manager API via raw FFI (no external crates).
//! Uses RmShutdown(RmForceShutdown) — same approach as rqbit.
//! Terminates ALL locking processes (no exclusions).

use crate::logger;
use crate::safety;

use std::fs;
use std::path::Path;

// ============================================================
// Win32 type definitions and FFI declarations
// ============================================================

#[allow(non_camel_case_types)]
type DWORD = u32;
#[allow(non_camel_case_types)]
type WCHAR = u16;
#[allow(non_camel_case_types)]
type LPCWSTR = *const u16;
#[allow(non_camel_case_types)]
type UINT = u32;

const ERROR_MORE_DATA: DWORD = 234;
const RM_FORCE_SHUTDOWN: DWORD = 1;
const CCH_RM_SESSION_KEY: usize = 32;

#[link(name = "rstrtmgr")]
extern "system" {
    fn RmStartSession(
        pSessionHandle: *mut DWORD,
        dwSessionFlags: DWORD,
        strSessionKey: *mut WCHAR,
    ) -> DWORD;

    fn RmEndSession(dwSessionHandle: DWORD) -> DWORD;

    fn RmRegisterResources(
        dwSessionHandle: DWORD,
        nFiles: UINT,
        rgsFileNames: *const LPCWSTR,
        nApplications: UINT,
        rgApplications: *const std::ffi::c_void,
        nServices: UINT,
        rgsServiceNames: *const LPCWSTR,
    ) -> DWORD;

    fn RmGetList(
        dwSessionHandle: DWORD,
        pnProcInfoNeeded: *mut UINT,
        pnProcInfo: *mut UINT,
        rgAffectedApps: *mut std::ffi::c_void,
        lpdwRebootReasons: *mut DWORD,
    ) -> DWORD;

    fn RmShutdown(
        dwSessionHandle: DWORD,
        lActionFlags: DWORD,
        fnStatus: *const std::ffi::c_void,
    ) -> DWORD;
}

// ============================================================
// RAII guard for Restart Manager session
// ============================================================

struct RmSessionGuard(DWORD);

impl Drop for RmSessionGuard {
    fn drop(&mut self) {
        unsafe {
            RmEndSession(self.0);
        }
    }
}

// ============================================================
// Helper functions
// ============================================================

/// Convert a Rust string to a null-terminated UTF-16 wide string.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Collect all file paths recursively from a directory.
fn collect_files(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files);
    files
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files);
        } else if let Some(s) = path.to_str() {
            files.push(s.to_string());
        }
    }
}

// ============================================================
// Main unlock function
// ============================================================

/// Run the unlock operation.
pub fn run(dir_path: &str) {
    let dir = Path::new(dir_path);

    // Safety guard
    if !safety::check_depth(dir, 3) {
        logger::log(&format!(
            "UNLOCK {:?} — path too shallow, aborted",
            dir_path
        ));
        std::process::exit(1);
    }

    if !dir.exists() {
        logger::log(&format!(
            "UNLOCK {:?} — directory does not exist, skipped",
            dir_path
        ));
        return;
    }

    // Collect all file paths
    let file_paths = collect_files(dir);
    if file_paths.is_empty() {
        logger::log(&format!(
            "UNLOCK {:?} — no files found, skipped",
            dir_path
        ));
        return;
    }

    // Convert to wide strings
    let wide_paths: Vec<Vec<u16>> = file_paths.iter().map(|p| to_wide(p)).collect();
    let wide_ptrs: Vec<LPCWSTR> = wide_paths.iter().map(|w| w.as_ptr()).collect();

    unsafe {
        // Step 1: Start Restart Manager session
        let mut session_handle: DWORD = 0;
        let mut session_key = [0u16; CCH_RM_SESSION_KEY + 1];

        let result = RmStartSession(
            &mut session_handle,
            0,
            session_key.as_mut_ptr(),
        );
        if result != 0 {
            logger::log(&format!(
                "UNLOCK {:?} — RmStartSession failed (error {})",
                dir_path, result
            ));
            return;
        }

        // RAII guard ensures RmEndSession is called even on error/panic
        let _guard = RmSessionGuard(session_handle);

        // Step 2: Register all files with Restart Manager
        let result = RmRegisterResources(
            session_handle,
            wide_ptrs.len() as UINT,
            wide_ptrs.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        );
        if result != 0 {
            logger::log(&format!(
                "UNLOCK {:?} — RmRegisterResources failed (error {})",
                dir_path, result
            ));
            return;
        }

        // Step 3: Query for locking processes (just to get count for logging)
        let mut reason: DWORD = 0;
        let mut n_proc_info_needed: UINT = 0;
        let mut n_proc_info: UINT = 0;

        let result = RmGetList(
            session_handle,
            &mut n_proc_info_needed,
            &mut n_proc_info,
            std::ptr::null_mut(),
            &mut reason,
        );

        if result == 0 && n_proc_info_needed == 0 {
            logger::log(&format!(
                "UNLOCK {:?} — no locking processes found",
                dir_path
            ));
            return;
        }

        if result != ERROR_MORE_DATA && result != 0 {
            logger::log(&format!(
                "UNLOCK {:?} — RmGetList failed (error {})",
                dir_path, result
            ));
            return;
        }

        let count = n_proc_info_needed;

        // Step 4: RmShutdown — let Restart Manager terminate all locking processes
        // Flag 1 = RmForceShutdown: graceful first, then force if needed
        let result = RmShutdown(
            session_handle,
            RM_FORCE_SHUTDOWN,
            std::ptr::null(),
        );

        if result == 0 {
            logger::log(&format!(
                "UNLOCK {:?} — terminated {} locking process(es)",
                dir_path, count
            ));
        } else {
            logger::log(&format!(
                "UNLOCK {:?} — RmShutdown failed (error {}), {} process(es) may still be locking",
                dir_path, result, count
            ));
        }

        // RmEndSession is called automatically by _guard Drop
    }
}
