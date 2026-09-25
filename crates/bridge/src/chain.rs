//! Running the previously configured status-line command ("chaining").
//!
//! Claude Code on Windows runs `statusLine.command` through Git Bash when it is installed, otherwise
//! through PowerShell. The bridge reproduces that so a chained command behaves exactly as before.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Set on the chained child so a bridge that ends up chaining to itself does not recurse.
pub const DEPTH_ENV: &str = "CUW_BRIDGE_CHAINED";
/// Test/diagnostic override of the shell: `bash`, `powershell` or `cmd`.
pub const SHELL_ENV: &str = "CUW_CHAIN_SHELL";

#[derive(Debug, PartialEq)]
pub enum ChainError {
    Spawn,
    Timeout,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shell {
    Bash(PathBuf),
    PowerShell,
    Cmd,
}

impl Shell {
    fn command(&self, cmd: &str) -> Command {
        match self {
            Shell::Bash(bash) => {
                let mut c = Command::new(bash);
                c.args(["-c", cmd]);
                c
            }
            Shell::PowerShell => {
                let mut c = Command::new("powershell.exe");
                c.args(["-NoProfile", "-NonInteractive", "-Command", cmd]);
                c
            }
            Shell::Cmd => {
                let mut c = Command::new("cmd.exe");
                c.args(["/D", "/C", cmd]);
                c
            }
        }
    }
}

/// Git Bash as Claude Code would find it: `CLAUDE_CODE_GIT_BASH_PATH`, else next to a `git.exe` on
/// `PATH`, else the default install location. Never WSL's `System32\bash.exe`.
pub fn find_git_bash() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join("git.exe").is_file() {
                // <Git>\cmd\git.exe or <Git>\bin\git.exe or <Git>\mingw64\bin\git.exe
                if let Some(parent) = dir.parent() {
                    candidates.push(parent.join("bin").join("bash.exe"));
                    if let Some(gp) = parent.parent() {
                        candidates.push(gp.join("bin").join("bash.exe"));
                    }
                }
            }
        }
    }
    for base in ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"] {
        if let Some(b) = std::env::var_os(base).map(PathBuf::from) {
            candidates.push(b.join("Git").join("bin").join("bash.exe"));
            candidates.push(b.join("Programs").join("Git").join("bin").join("bash.exe"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

pub fn resolve_shell() -> Shell {
    match std::env::var(SHELL_ENV).ok().as_deref() {
        Some("powershell") => return Shell::PowerShell,
        Some("cmd") => return Shell::Cmd,
        _ => {}
    }
    match find_git_bash() {
        Some(b) => Shell::Bash(b),
        None => Shell::PowerShell,
    }
}

/// Run `cmd` with `stdin` piped in; return its stdout, or an error on spawn failure or timeout.
/// On timeout the shell process is killed (grandchildren may linger holding the pipe; the reader
/// thread is abandoned and the bridge exits right after).
pub fn run_chained(cmd: &str, stdin: &[u8], timeout: Duration) -> Result<Vec<u8>, ChainError> {
    run_with_shell(&resolve_shell(), cmd, stdin, timeout)
}

pub fn run_with_shell(
    shell: &Shell,
    cmd: &str,
    stdin: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, ChainError> {
    let mut command = shell.command(cmd);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env(DEPTH_ENV, "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(windows)]
    crate::win::make_std_handles_non_inheritable();
    let mut child = command.spawn().map_err(|_| ChainError::Spawn)?;
    #[cfg(windows)]
    let job = crate::win::KillJob::new().filter(|j| j.assign(&child));

    if let Some(mut sin) = child.stdin.take() {
        let data = stdin.to_vec();
        std::thread::spawn(move || {
            let _ = sin.write_all(&data);
        });
    }
    let mut sout = child.stdout.take().ok_or(ChainError::Spawn)?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = sout.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    match rx.recv_timeout(timeout) {
        Ok(out) => {
            let _ = child.wait();
            Ok(out)
        }
        Err(_) => {
            #[cfg(windows)]
            if let Some(j) = &job {
                j.terminate();
            }
            let _ = child.kill();
            let _ = child.wait();
            Err(ChainError::Timeout)
        }
    }
}
