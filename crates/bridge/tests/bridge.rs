//! End-to-end tests: run the real `cuw-bridge` executable against a temporary data directory.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use cuw_bridge::fsio::{write_atomic, FileLock};
use cuw_core::bridge_state::{parse_history, BridgeState};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("cuw-test-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Run {
    code: i32,
    stdout: String,
    elapsed: Duration,
}

fn bridge(dir: &Path, stdin: &str) -> Run {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_cuw-bridge"))
        .env("CUW_DATA_DIR", dir)
        .env_remove("CUW_BRIDGE_CHAINED")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        elapsed: start.elapsed(),
    }
}

fn state(dir: &Path) -> BridgeState {
    BridgeState::from_json(&std::fs::read_to_string(dir.join("state.json")).unwrap()).unwrap()
}

fn payload(five: f64, seven: f64) -> String {
    let future = cuw_bridge::now_secs() + 3600;
    format!(
        r#"{{"model":{{"display_name":"Opus"}},"rate_limits":{{"five_hour":{{"used_percentage":{five},"resets_at":{future}}},"seven_day":{{"used_percentage":{seven},"resets_at":{}}}}}}}"#,
        future + 86_400
    )
}

fn set_previous_command(dir: &Path, cmd: &str) {
    let rec = serde_json::json!({ "previous_status_line": { "type": "command", "command": cmd } });
    std::fs::write(dir.join("install.json"), rec.to_string()).unwrap();
}

#[test]
fn empty_stdin_exits_zero_with_default_line() {
    let d = TempDir::new("empty");
    let r = bridge(d.path(), "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "5h -- \u{b7} 7d --\n");
    assert!(!d.path().join("state.json").exists());
}

#[test]
fn malformed_json_exits_zero() {
    let d = TempDir::new("malformed");
    for input in ["{", "not json at all", "[]", "\u{0}\u{1}garbage"] {
        let r = bridge(d.path(), input);
        assert_eq!(r.code, 0, "{input:?}");
        assert_eq!(r.stdout, "5h -- \u{b7} 7d --\n");
    }
}

#[test]
fn payload_writes_state_history_and_line() {
    let d = TempDir::new("payload");
    let r = bridge(d.path(), &payload(23.5, 41.2));
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "5h 24% \u{b7} 7d 41%\n");
    let st = state(d.path());
    assert_eq!(st.windows["five_hour"].used_percentage, 23.5);
    assert_eq!(st.windows["seven_day"].used_percentage, 41.2);
    let hist = parse_history(&std::fs::read_to_string(d.path().join("history.jsonl")).unwrap());
    assert_eq!(hist.len(), 2);
    // No temp or lock file left behind.
    let leftovers: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp") || n.ends_with(".lock"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn missing_rate_limits_keeps_state_and_shows_it() {
    let d = TempDir::new("norl");
    bridge(d.path(), &payload(10.0, 20.0));
    let before = std::fs::read_to_string(d.path().join("state.json")).unwrap();
    let r = bridge(d.path(), r#"{"model":{"display_name":"Opus"}}"#);
    assert_eq!(r.stdout, "5h 10% \u{b7} 7d 20%\n");
    assert_eq!(
        std::fs::read_to_string(d.path().join("state.json")).unwrap(),
        before
    );
}

#[test]
fn concurrent_sessions_keep_the_maximum() {
    let d = TempDir::new("concurrent");
    let future = cuw_bridge::now_secs() + 3600;
    let handles: Vec<_> = (1..=8)
        .map(|i| {
            let dir = d.path().to_path_buf();
            let p = format!(r#"{{"rate_limits":{{"five_hour":{{"used_percentage":{},"resets_at":{future}}}}}}}"#, i * 5);
            std::thread::spawn(move || bridge(&dir, &p).code)
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), 0);
    }
    assert_eq!(state(d.path()).windows["five_hour"].used_percentage, 40.0);
}

#[test]
fn chains_previous_command_with_same_stdin() {
    let d = TempDir::new("chain");
    // Echoes the model name it received on stdin, proving the payload was forwarded.
    set_previous_command(d.path(), r#"grep -o '"display_name":"[^"]*"' | head -n 1"#);
    let r = bridge(d.path(), &payload(1.0, 2.0));
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout.trim(), r#""display_name":"Opus""#);
    // State is still recorded when chaining.
    assert_eq!(state(d.path()).windows["five_hour"].used_percentage, 1.0);
}

#[test]
fn chained_command_timeout_falls_back_to_default_line() {
    let d = TempDir::new("timeout");
    set_previous_command(d.path(), "sleep 10; echo too-late");
    let r = bridge(d.path(), &payload(7.0, 8.0));
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "5h 7% \u{b7} 7d 8%\n");
    assert!(r.elapsed < Duration::from_secs(5), "took {:?}", r.elapsed);
}

#[test]
fn failing_or_silent_chained_command_falls_back() {
    let d = TempDir::new("fail");
    for cmd in ["definitely-not-a-command-xyz", "exit 3", "true"] {
        set_previous_command(d.path(), cmd);
        let r = bridge(d.path(), &payload(3.0, 4.0));
        assert_eq!(r.code, 0, "{cmd}");
        assert_eq!(r.stdout, "5h 3% \u{b7} 7d 4%\n", "{cmd}");
    }
}

#[test]
fn chaining_to_itself_does_not_recurse() {
    let d = TempDir::new("self");
    let exe = env!("CARGO_BIN_EXE_cuw-bridge").replace('\\', "/");
    set_previous_command(d.path(), &format!("\"{exe}\""));
    let r = bridge(d.path(), &payload(5.0, 6.0));
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "5h 5% \u{b7} 7d 6%\n");
    assert!(r.elapsed < Duration::from_secs(3));
}

#[test]
fn atomic_write_readers_never_see_partial_content() {
    let d = TempDir::new("atomic");
    let path = d.path().join("state.json");
    write_atomic(&path, b"{\"n\":0}").unwrap();
    let reader_path = path.clone();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_r = stop.clone();
    let reader = std::thread::spawn(move || {
        let mut reads = 0;
        while !stop_r.load(Ordering::SeqCst) {
            if let Ok(s) = std::fs::read_to_string(&reader_path) {
                let v: serde_json::Value =
                    serde_json::from_str(&s).expect("partial or corrupt read");
                assert!(v["pad"].as_str().is_none_or(|p| p.len() == 20_000));
                reads += 1;
            }
        }
        reads
    });
    for n in 1..=200 {
        let body = serde_json::json!({ "n": n, "pad": "x".repeat(20_000) });
        write_atomic(&path, body.to_string().as_bytes()).unwrap();
    }
    stop.store(true, Ordering::SeqCst);
    assert!(reader.join().unwrap() > 0);
    let last: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(last["n"], 200);
    let tmp_left = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
    assert!(!tmp_left);
}

#[test]
fn lock_is_exclusive_and_released() {
    let d = TempDir::new("lock");
    let p = d.path().join("state.lock");
    let a = FileLock::acquire(&p);
    assert!(a.held());
    let b = FileLock::acquire(&p);
    assert!(
        !b.held(),
        "second lock must not be granted while the first is held"
    );
    drop(a);
    assert!(!p.exists());
    assert!(FileLock::acquire(&p).held());
}
