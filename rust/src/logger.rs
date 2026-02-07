//! Simple log file writer.
//!
//! Appends timestamped lines to `zDirComp.log` next to the executable.
//! All errors are silently ignored (best-effort logging).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

/// Get the log file path (next to the executable).
fn log_path() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("zDirComp.log")))
}

/// Format current local time as `[YYYY-MM-DD HH:MM:SS]`.
fn timestamp() -> String {
    let now = SystemTime::now();
    let since_epoch = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = since_epoch.as_secs();

    // Get local time offset from Windows API
    let offset_secs = local_utc_offset_secs();
    let local_secs = total_secs as i64 + offset_secs;

    let days = local_secs / 86400;
    let time_of_day = ((local_secs % 86400) + 86400) % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Convert days since epoch to Y-M-D (civil calendar)
    let (year, month, day) = days_to_ymd(days);

    format!(
        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}]",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    // Algorithm from Howard Hinnant's chrono-compatible date algorithms
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Get local UTC offset in seconds using Win32 API.
fn local_utc_offset_secs() -> i64 {
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        _standard_name: [u16; 32],
        _standard_date: [u16; 8],
        _standard_bias: i32,
        _daylight_name: [u16; 32],
        _daylight_date: [u16; 8],
        _daylight_bias: i32,
    }

    extern "system" {
        fn GetTimeZoneInformation(tz: *mut TimeZoneInformation) -> u32;
    }

    unsafe {
        let mut tz = std::mem::zeroed::<TimeZoneInformation>();
        GetTimeZoneInformation(&mut tz);
        // Bias is in minutes, UTC = local + bias, so local = UTC - bias
        -(tz.bias as i64) * 60
    }
}

/// Append a log line to the log file.
pub fn log(message: &str) {
    if let Some(path) = log_path() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{} {}", timestamp(), message);
        }
    }
}
