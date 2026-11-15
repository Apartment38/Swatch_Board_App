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
