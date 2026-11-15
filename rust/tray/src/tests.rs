use super::*;

fn child(script: &str) -> tokio::process::Child {
    Command::new("sh")
        .args(["-c", script])
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

#[tokio::test]
async fn distinguishes_selection_cancellation_crash_and_invalid_output() {
    let (_tx, rx) = watch::channel(false);
    let timeout = Duration::from_secs(3);
    assert!(
        matches!(supervise(child("printf '#12aBcD\\n'"), rx.clone(), timeout).await, Outcome::Selected(value) if value == "#12ABCD")
    );
    assert!(matches!(
        supervise(child("exit 0"), rx.clone(), timeout).await,
        Outcome::Cancelled
    ));
    assert!(
        matches!(supervise(child("exit 4"), rx.clone(), timeout).await, Outcome::Failed(reason) if reason.contains("abnormally"))
    );
    assert!(
        matches!(supervise(child("printf invalid"), rx, timeout).await, Outcome::Failed(reason) if reason.contains("invalid"))
    );
}

#[tokio::test]
async fn timeout_and_shutdown_reap_picker() {
    let (tx, rx) = watch::channel(false);
    let running = child("exec sleep 30");
    let pid = running.id().unwrap();
    assert!(
        matches!(supervise(running, rx.clone(), Duration::from_millis(50)).await, Outcome::Failed(reason) if reason.contains("timed out"))
    );
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    let running = child("exec sleep 30");
    let pid = running.id().unwrap();
    let task = tokio::spawn(supervise(running, rx, Duration::from_secs(30)));
    tx.send(true).unwrap();
    assert!(matches!(task.await.unwrap(), Outcome::Shutdown));
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}

#[tokio::test]
async fn timeout_and_shutdown_allow_picker_cleanup() {
    for shutdown_picker in [false, true] {
        let mut running = Command::new("sh")
            .args([
                "-c",
                "trap 'printf cleanup >&2; exit 0' TERM; printf R; read -r line",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let _stdin = running.stdin.take().unwrap();
        let mut stdout = running.stdout.take().unwrap();
        let mut ready = [0];
        tokio::time::timeout(Duration::from_secs(3), stdout.read_exact(&mut ready))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ready, *b"R");
        running.stdout = Some(stdout);
        let mut stderr = running.stderr.take().unwrap();
        let (tx, rx) = watch::channel(false);
        let timeout = if shutdown_picker {
            Duration::from_secs(30)
        } else {
            Duration::from_millis(50)
        };
        let task = tokio::spawn(supervise(running, rx, timeout));
        if shutdown_picker {
            tx.send(true).unwrap();
        }
        let outcome = task.await.unwrap();
        if shutdown_picker {
            assert!(matches!(outcome, Outcome::Shutdown));
        } else {
            assert!(matches!(outcome, Outcome::Failed(reason) if reason.contains("timed out")));
        }
        let mut cleanup = Vec::new();
        stderr.read_to_end(&mut cleanup).await.unwrap();
        assert_eq!(cleanup, b"cleanup");
    }
}
