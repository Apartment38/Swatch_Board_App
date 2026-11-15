//! These tests use Cargo's exact helper artifact; unit/Board render tests also
//! require `cargo build -p swatch-board-theme-core --bin swatch-board-theme-worker`.
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn worker() -> Worker {
    Worker(
        Command::new(env!("CARGO_BIN_EXE_swatch-board-theme-worker"))
            .arg("--protocol-v1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}
#[test]
fn limits_are_active_before_reading_request() {
    let child = worker();
    let path = format!("/proc/{}/limits", child.0.id());
    let start = Instant::now();
    loop {
        let limits = std::fs::read_to_string(&path).unwrap();
        let words = |name: &str| {
            limits
                .lines()
                .find(|l| l.starts_with(name))
                .unwrap()
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let address = words("Max address space");
        if address[3] == "67108864" {
            assert_eq!(address[4], "67108864");
            let cpu = words("Max cpu time");
            assert_eq!(&cpu[3..5], &["2", "2"]);
            let core = words("Max core file size");
            assert_eq!(&core[4..6], &["0", "0"]);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(1));
    }
    // The request pipe is still open: no request was read/deserialized to set limits.
}
#[test]
fn oversized_and_malformed_requests_fail_without_protocol_output() {
    for payload in [b"not json".to_vec(), vec![b' '; 16 * 1024 * 1024 + 1]] {
        let mut child = worker();
        let _ = child.0.stdin.take().unwrap().write_all(&payload);
        let start = Instant::now();
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut bytes = Vec::new();
        child
            .0
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.is_empty());
    }
}
