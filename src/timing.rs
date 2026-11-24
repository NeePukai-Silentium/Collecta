//! Startup time and memory, written to the log given with `--log <file>`.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetSystemTimeAsFileTime};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn set_log_path(path: PathBuf) {
    let _ = LOG_PATH.set(path);
}

fn ticks(time: FILETIME) -> u64 {
    ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64
}

/// Milliseconds since Windows created this process, including the time before `main`.
pub fn since_process_start() -> u64 {
    unsafe {
        let mut created = FILETIME::default();
        let mut unused = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        if GetProcessTimes(GetCurrentProcess(), &mut created, &mut unused.0, &mut unused.1, &mut unused.2).is_err() {
            return 0;
        }
        ticks(GetSystemTimeAsFileTime()).saturating_sub(ticks(created)) / 10_000
    }
}

/// "private 27.1 MB, working set 39.0 MB"
pub fn memory_usage() -> String {
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let _ = GetProcessMemoryInfo(GetCurrentProcess(), &mut counters as *mut _ as *mut _, counters.cb);
        let mb = |bytes: usize| bytes as f64 / (1024.0 * 1024.0);
        format!("private {:.1} MB, working set {:.1} MB", mb(counters.PrivateUsage), mb(counters.WorkingSetSize))
    }
}

/// Does nothing without `--log`.
pub fn write_log(line: &str) {
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let now = unsafe { GetLocalTime() };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}  {line}",
            now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
        );
    }
}
