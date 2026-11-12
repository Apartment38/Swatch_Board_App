//! Resource containment, not a filesystem/network sandbox. No fork callbacks:
//! Command execs a fresh helper which lowers its own limits before parsing.
use super::*;
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const WORKER: &str = "swatch-board-theme-worker";
const REQUEST_LIMIT: usize = 16 * 1024 * 1024;
const RESPONSE_LIMIT: usize = 16 * 1024 * 1024;
const TOTAL_OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
const WALL_LIMIT: Duration = Duration::from_secs(3);
const ADDRESS_SPACE: libc::rlim_t = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    version: u32,
    pub outputs: Vec<Output>,
    pub templates: BTreeMap<String, String>,
    pub roles: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    version: u32,
    files: Vec<RenderedFile>,
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("worker message too large"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize, limit: usize) -> Result<Vec<u8>, String> {
    let mut out = BoundedBuffer {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut out, value).map_err(|e| e.to_string())?;
    Ok(out.bytes)
}
fn worker_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("no executable directory")?;
    let sibling = dir.join(WORKER);
    if sibling.is_file() {
        return Ok(sibling);
    }
    // Cargo unit/integration tests live under target/{profile}/deps.
    #[cfg(debug_assertions)]
    if dir.file_name() == Some(OsStr::new("deps")) {
        let candidate = dir
            .parent()
            .ok_or("no Cargo profile directory")?
            .join(WORKER);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "missing {WORKER} beside executable; build/install the rendering helper"
    ))
}

// Every return, including IO/protocol errors, kills and waits. Successful children
// have already been waited by try_wait; a redundant wait is harmless.
struct Reap(Child);
impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn nonblocking(fd: i32) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(())
}
fn exchange(path: &Path, request: &[u8], deadline: Duration) -> Result<Vec<u8>, String> {
    let started = Instant::now();
    let child = Command::new(path)
        .arg("--protocol-v1")
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("start theme worker: {e}"))?;
    exchange_child(child, request, started, deadline)
}
fn exchange_child(
    child: Child,
    request: &[u8],
    started: Instant,
    deadline: Duration,
) -> Result<Vec<u8>, String> {
    let mut child = Reap(child);
    let mut input = child.0.stdin.take();
    let mut output = child.0.stdout.take().ok_or("worker stdout missing")?;
    nonblocking(input.as_ref().ok_or("worker stdin missing")?.as_raw_fd())?;
    nonblocking(output.as_raw_fd())?;
    let mut sent = 0;
    let mut bytes = Vec::new();
    let mut eof = false;
    loop {
        if started.elapsed() >= deadline {
            return Err("theme worker wall-clock timeout".into());
        }
        if let Some(pipe) = &mut input {
            match pipe.write(&request[sent..]) {
                Ok(0) => return Err("theme worker closed request pipe".into()),
                Ok(n) => sent += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(format!("theme worker request: {e}")),
            }
            if sent == request.len() {
                input = None;
            }
        }
        // One bounded read per iteration keeps both IO directions and the deadline live.
        let mut chunk = [0u8; 8192];
        match output.read(&mut chunk) {
            Ok(0) => eof = true,
            Ok(n) => {
                if bytes.len() + n > RESPONSE_LIMIT {
                    return Err("theme worker response too large".into());
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(format!("theme worker response: {e}")),
        }
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err(format!("theme worker failed: {status}"));
            }
            if eof {
                if sent != request.len() {
                    return Err("theme worker did not consume request".into());
                }
                return Ok(bytes);
            }
        }
        // Avoid busy waiting when both pipes would block, without serializing IO.
        if !eof {
            std::thread::sleep(Duration::from_micros(100));
        }
    }
}
fn validate_files(outputs: &[Output], files: &[RenderedFile]) -> Result<(), String> {
    if files.len() != outputs.len() || files.is_empty() || files.len() > MAX_OUTPUTS {
        return Err("theme worker output count mismatch".into());
    }
    let mut total = 0usize;
    for (declared, file) in outputs.iter().zip(files) {
        parts(&file.path)?;
        total = total.saturating_add(file.content.len());
        if file.path != declared.path
            || file.content.len() > OUTPUT_LIMIT
            || total > TOTAL_OUTPUT_LIMIT
        {
            return Err("theme worker invalid path or oversized output".into());
        }
        validate_output(&declared.format, &file.content)?;
    }
    Ok(())
}
pub(super) fn render(
    outputs: &[Output],
    templates: &BTreeMap<String, String>,
    roles: BTreeMap<String, String>,
) -> Result<Vec<RenderedFile>, String> {
    // Bound cloning/serialization too, even if public manifest data was mutated.
    if outputs.len() > MAX_OUTPUTS
        || templates.len() > MAX_OUTPUTS
        || roles.len() > 1024
        || templates
            .iter()
            .any(|(k, v)| k.len() > 1024 || v.len() > DATA_LIMIT)
    {
        return Err("theme worker request limits exceeded".into());
    }
    let request = Request {
        version: 1,
        outputs: outputs.to_vec(),
        templates: templates.clone(),
        roles,
    };
    let payload = encode(&request, REQUEST_LIMIT)?;
    let bytes = exchange(&worker_path()?, &payload, WALL_LIMIT)?;
    let response: Response = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid theme worker response: {e}"))?;
    if response.version != 1 {
        return Err("unsupported theme worker response version".into());
    }
    validate_files(outputs, &response.files)?;
    Ok(response.files)
}

pub(super) fn worker_main() -> Result<(), String> {
    // These are set in the fresh single-threaded executable, never after fork in Board.
    for (resource, value) in [
        (libc::RLIMIT_CORE, 0),
        (libc::RLIMIT_AS, ADDRESS_SPACE),
        (libc::RLIMIT_CPU, 2),
    ] {
        let limit = libc::rlimit {
            rlim_cur: value,
            rlim_max: value,
        };
        if unsafe { libc::setrlimit(resource, &limit) } != 0 {
            return Err(io::Error::last_os_error().to_string());
        }
    }
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--protocol-v1"] {
        return Err("unsupported worker invocation".into());
    }
    let mut bytes = Vec::new();
    io::stdin()
        .take(REQUEST_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > REQUEST_LIMIT {
        return Err("theme worker request too large".into());
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if request.version != 1
        || request.outputs.is_empty()
        || request.outputs.len() > MAX_OUTPUTS
        || request.templates.len() > MAX_OUTPUTS
        || request.roles.len() > 1024
        || request
            .templates
            .iter()
            .any(|(k, v)| parts(k).is_err() || v.len() > DATA_LIMIT)
    {
        return Err("invalid theme worker request".into());
    }
    let files = super::render_contained(&request)?;
    validate_files(&request.outputs, &files)?;
    let response = encode(&Response { version: 1, files }, RESPONSE_LIMIT)?;
    io::stdout().write_all(&response).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Isolated test subprocess, not a shell and not a production worker mode.
    #[test]
    #[ignore]
    fn pipe_fixture() {
        match std::env::var("THEME_PIPE_FIXTURE").unwrap().as_str() {
            "stall" => std::thread::sleep(Duration::from_secs(30)),
            "flood" => {
                let chunk = [b'x'; 8192];
                loop {
                    if io::stdout().write_all(&chunk).is_err() {
                        break;
                    }
                }
            }
            "duplex" => {
                // Emit beyond pipe capacity before consuming a similarly large request.
                io::stdout().write_all(&vec![b'x'; 256 * 1024]).unwrap();
                let mut bytes = Vec::new();
                io::stdin().read_to_end(&mut bytes).unwrap();
                assert_eq!(bytes.len(), 256 * 1024);
            }
            _ => panic!("unknown fixture"),
        }
    }
    fn fixture(mode: &str) -> Child {
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "isolation::tests::pipe_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("THEME_PIPE_FIXTURE", mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }
    fn assert_reaped(pid: u32) {
        let mut status = 0;
        assert_eq!(
            unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) },
            -1
        );
        assert_eq!(
            io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
    }
    #[test]
    fn stalled_request_writer_times_out_and_reaps() {
        let child = fixture("stall");
        let pid = child.id();
        let started = Instant::now();
        let error = exchange_child(
            child,
            &vec![0; 256 * 1024],
            started,
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(error.contains("timeout"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_reaped(pid);
    }
    #[test]
    fn oversized_response_is_bounded_and_reaped() {
        let child = fixture("flood");
        let pid = child.id();
        let error = exchange_child(child, b"{}", Instant::now(), WALL_LIMIT).unwrap_err();
        assert!(error.contains("response too large"), "{error}");
        assert_reaped(pid);
    }
    #[test]
    fn simultaneous_large_request_and_response_do_not_deadlock() {
        let child = fixture("duplex");
        let pid = child.id();
        let bytes =
            exchange_child(child, &vec![0; 256 * 1024], Instant::now(), WALL_LIMIT).unwrap();
        assert!(bytes.len() >= 256 * 1024);
        assert_reaped(pid);
    }
    #[test]
    fn malformed_and_mismatched_results_are_rejected() {
        assert!(serde_json::from_slice::<Response>(b"garbage").is_err());
        assert!(
            serde_json::from_slice::<Response>(br#"{"version":1,"files":[],"extra":1}"#).is_err()
        );
        let outputs = vec![Output {
            path: "theme".into(),
            template: "templates/a.j2".into(),
            format: "json".into(),
        }];
        for file in [
            RenderedFile {
                path: "../escape".into(),
                content: "{}".into(),
            },
            RenderedFile {
                path: "other".into(),
                content: "{}".into(),
            },
            RenderedFile {
                path: "theme".into(),
                content: "invalid json".into(),
            },
        ] {
            assert!(validate_files(&outputs, &[file]).is_err());
        }
        assert!(validate_files(&outputs, &[]).is_err());
        assert!(encode(&"12345", 4).is_err());
    }
    #[test]
    fn per_file_and_aggregate_output_caps_are_enforced() {
        let output = Output {
            path: "theme".into(),
            template: "templates/a.j2".into(),
            format: "text".into(),
        };
        let oversized = RenderedFile {
            path: "theme".into(),
            content: "x".repeat(OUTPUT_LIMIT + 1),
        };
        assert!(validate_files(std::slice::from_ref(&output), &[oversized]).is_err());
        let outputs: Vec<_> = (0..3)
            .map(|i| Output {
                path: format!("theme{i}"),
                ..output.clone()
            })
            .collect();
        let files: Vec<_> = outputs
            .iter()
            .map(|o| RenderedFile {
                path: o.path.clone(),
                content: "x".repeat(3 * 1024 * 1024),
            })
            .collect();
        assert!(validate_files(&outputs, &files).is_err());
    }
    #[test]
    fn missing_executable_fails_closed() {
        assert!(
            exchange(
                Path::new("/nonexistent/swatch-board-theme-worker"),
                b"{}",
                WALL_LIMIT
            )
            .is_err()
        );
    }
}
