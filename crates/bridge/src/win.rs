//! Minimal Win32 FFI (no extra crates) for chaining safely:
//! - the bridge's own stdio handles are made non-inheritable, so a lingering grandchild of the chained
//!   command cannot hold Claude Code's pipe open;
//! - the chained command runs in a job object that kills its whole process tree on timeout or when
//!   the bridge exits.

use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

type Handle = *mut c_void;

const STD_INPUT_HANDLE: u32 = -10i32 as u32;
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
const STD_ERROR_HANDLE: u32 = -12i32 as u32;
const HANDLE_FLAG_INHERIT: u32 = 0x1;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

#[repr(C)]
#[derive(Default)]
struct BasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[derive(Default)]
struct IoCounters {
    counters: [u64; 6],
}

#[repr(C)]
#[derive(Default)]
struct ExtendedLimitInformation {
    basic: BasicLimitInformation,
    io: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(n: u32) -> Handle;
    fn SetHandleInformation(h: Handle, mask: u32, flags: u32) -> i32;
    fn CreateJobObjectW(attrs: *mut c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: i32, info: *mut c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
    fn CloseHandle(h: Handle) -> i32;
}

/// Stop the bridge's own stdin/stdout/stderr from being inherited by processes it spawns.
pub fn make_std_handles_non_inheritable() {
    for n in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: plain Win32 calls on handles owned by this process; failures are ignored.
        unsafe {
            let h = GetStdHandle(n);
            if !h.is_null() && h as isize != -1 {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

/// A job object that kills every process in it when terminated or dropped.
pub struct KillJob(Handle);

impl KillJob {
    pub fn new() -> Option<KillJob> {
        // SAFETY: creating an anonymous job object and setting a documented limit structure.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let mut info = ExtendedLimitInformation::default();
            info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                &mut info as *mut _ as *mut c_void,
                std::mem::size_of::<ExtendedLimitInformation>() as u32,
            );
            if ok == 0 {
                CloseHandle(job);
                return None;
            }
            Some(KillJob(job))
        }
    }

    pub fn assign(&self, child: &Child) -> bool {
        // SAFETY: the child's process handle is valid while `child` lives.
        unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as Handle) != 0 }
    }

    pub fn terminate(&self) {
        // SAFETY: valid job handle owned by self.
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

impl Drop for KillJob {
    fn drop(&mut self) {
        // SAFETY: closing our own handle; KILL_ON_JOB_CLOSE ends any remaining processes.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
