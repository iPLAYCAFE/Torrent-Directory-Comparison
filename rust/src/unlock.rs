//! Mode 2: Unlock — kill processes locking files in the torrent directory.
//!
//! Uses Win32 Restart Manager API via raw FFI (no external crates).
//! Excludes uTorrent.exe and BitTorrent.exe from termination.

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
type BOOL = i32;
#[allow(non_camel_case_types)]
type HANDLE = *mut std::ffi::c_void;
#[allow(non_camel_case_types)]
type LPCWSTR = *const u16;
#[allow(non_camel_case_types)]
type UINT = u32;

const ERROR_MORE_DATA: DWORD = 234;
const PROCESS_TERMINATE: DWORD = 0x0001;
const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;
const CCH_RM_SESSION_KEY: usize = 32; // Character count, +1 for null
const CCH_RM_MAX_APP_NAME: usize = 255;
const CCH_RM_MAX_SVC_NAME: usize = 63;

#[repr(C)]
#[derive(Clone)]
#[allow(non_snake_case)]
struct RM_UNIQUE_PROCESS {
    dwProcessId: DWORD,
    ProcessStartTime: u64, // FILETIME as u64
}

#[repr(C)]
#[derive(Clone)]
#[allow(non_snake_case)]
struct RM_PROCESS_INFO {
    Process: RM_UNIQUE_PROCESS,
    strAppName: [WCHAR; CCH_RM_MAX_APP_NAME + 1],
    strServiceShortName: [WCHAR; CCH_RM_MAX_SVC_NAME + 1],
    ApplicationType: DWORD,
    AppStatus: DWORD,
    TSSessionId: DWORD,
    bRestartable: BOOL,
}

impl Default for RM_PROCESS_INFO {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

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
        rgApplications: *const RM_UNIQUE_PROCESS,
        nServices: UINT,
        rgsServiceNames: *const LPCWSTR,
    ) -> DWORD;

    fn RmGetList(
        dwSessionHandle: DWORD,
        pnProcInfoNeeded: *mut UINT,
        pnProcInfo: *mut UINT,
        rgAffectedApps: *mut RM_PROCESS_INFO,
        lpdwRebootReasons: *mut DWORD,
    ) -> DWORD;
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
    fn TerminateProcess(hProcess: HANDLE, uExitCode: UINT) -> BOOL;
    fn CloseHandle(hObject: HANDLE) -> BOOL;
    fn QueryFullProcessImageNameW(
        hProcess: HANDLE,
        dwFlags: DWORD,
        lpExeName: *mut WCHAR,
        lpdwSize: *mut DWORD,
    ) -> BOOL;
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

/// Convert a null-terminated UTF-16 slice to a Rust String.
fn from_wide(s: &[u16]) -> String {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end])
}

/// Get the executable name of a process by PID.
fn get_process_exe_name(pid: DWORD) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut buf = [0u16; 1024];
        let mut size = buf.len() as DWORD;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);

        if ok != 0 {
            let full_path = from_wide(&buf[..size as usize]);
            // Extract just the filename
            full_path
                .rsplit('\\')
                .next()
                .map(|s| s.to_string())
        } else {
            None
        }
    }
}

/// Check if a process name is in the exclusion list (case-insensitive).
fn is_excluded(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "utorrent.exe" || lower == "bittorrent.exe"
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
        // Start Restart Manager session
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

        // RAII guard ensures RmEndSession is called
        let _guard = RmSessionGuard(session_handle);

        // Register files
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

        // Query for locking processes (first call to get count)
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

        // Second call to get actual process info
        n_proc_info = n_proc_info_needed;
        let mut proc_infos = vec![RM_PROCESS_INFO::default(); n_proc_info as usize];

        let result = RmGetList(
            session_handle,
            &mut n_proc_info_needed,
            &mut n_proc_info,
            proc_infos.as_mut_ptr(),
            &mut reason,
        );
        if result != 0 {
            logger::log(&format!(
                "UNLOCK {:?} — RmGetList (second call) failed (error {})",
                dir_path, result
            ));
            return;
        }

        // Process each locking process
        let mut killed = 0u32;
        let mut skipped_names = Vec::new();
        let mut killed_names = Vec::new();

        for i in 0..n_proc_info as usize {
            let pid = proc_infos[i].Process.dwProcessId;
            let app_name = from_wide(&proc_infos[i].strAppName);

            // Get the actual exe name for exclusion check
            let exe_name = get_process_exe_name(pid).unwrap_or_default();

            if is_excluded(&exe_name) {
                skipped_names.push(exe_name);
                continue;
            }

            // Terminate the process
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if !handle.is_null() {
                if TerminateProcess(handle, 1) != 0 {
                    killed += 1;
                    killed_names.push(if exe_name.is_empty() {
                        app_name.clone()
                    } else {
                        exe_name
                    });
                }
                CloseHandle(handle);
            }
        }

        // Log summary
        let mut msg = format!("UNLOCK {:?}", dir_path);
        if killed > 0 {
            msg.push_str(&format!(
                " — killed {} process(es) ({})",
                killed,
                killed_names.join(", ")
            ));
        } else {
            msg.push_str(" — no processes to terminate");
        }
        if !skipped_names.is_empty() {
            msg.push_str(&format!(", skipped {}", skipped_names.join(", ")));
        }
        logger::log(&msg);
    }
}
