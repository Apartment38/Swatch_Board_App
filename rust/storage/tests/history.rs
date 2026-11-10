use std::fs;

use rusqlite::Connection;
use swatch_board_storage::{CaptureSource, DEFAULT_RETENTION, Error, HistoryRepository, Rgba8};
use tempfile::tempdir;

fn color() -> Rgba8 {
    Rgba8 {
        r: 12,
        g: 34,
        b: 56,
        a: 78,
    }
}

#[test]
fn captures_survive_reopen_and_duplicates_are_distinct() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested/library.sqlite3");
    let mut history = HistoryRepository::open(&path).unwrap();
    let first = history.record(color(), CaptureSource::Picker).unwrap();
    let second = history.record(color(), CaptureSource::Board).unwrap();
    assert_ne!(first.id, second.id);
    drop(history);
    let history = HistoryRepository::open(&path).unwrap();
    assert_eq!(history.list().unwrap(), vec![second.clone(), first]);
    assert_eq!(history.last().unwrap(), Some(second));
}

#[test]
fn default_and_custom_retention_remove_oldest_only() {
    let dir = tempdir().unwrap();
    let mut history = HistoryRepository::open(dir.path().join("db")).unwrap();
    let first = history.record(color(), CaptureSource::Picker).unwrap();
    for _ in 0..DEFAULT_RETENTION {
        history.record(color(), CaptureSource::Picker).unwrap();
    }
    let entries = history.list().unwrap();
    assert_eq!(entries.len(), DEFAULT_RETENTION);
    assert!(!entries.iter().any(|entry| entry.id == first.id));
    history.set_retention(1);
    let newest = history.record(color(), CaptureSource::Board).unwrap();
    assert_eq!(history.list().unwrap(), vec![newest]);
    history.set_retention(0);
    history.record(color(), CaptureSource::Picker).unwrap();
    assert!(history.list().unwrap().is_empty());
}

#[test]
fn independent_connections_observe_commits_and_required_pragmas() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("db");
    let mut one = HistoryRepository::open(&path).unwrap();
    let two = HistoryRepository::open(&path).unwrap();
    let capture = one.record(color(), CaptureSource::Picker).unwrap();
    assert_eq!(two.last().unwrap(), Some(capture));
    let connection = Connection::open(&path).unwrap();
    let journal: String = connection
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(journal.to_lowercase(), "wal");
}

#[test]
fn failed_pruning_rolls_back_insert() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("db");
    let mut history = HistoryRepository::open(&path).unwrap();
    let first = history.record(color(), CaptureSource::Picker).unwrap();
    history.set_retention(1);
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER prevent_pruning BEFORE DELETE ON color_history BEGIN SELECT RAISE(ABORT, 'cannot prune'); END;").unwrap();
    assert!(history.record(color(), CaptureSource::Board).is_err());
    assert_eq!(history.list().unwrap(), vec![first]);
}

#[test]
fn rejects_newer_unversioned_and_malformed_databases() {
    let dir = tempdir().unwrap();
    let future = dir.path().join("future");
    let connection = Connection::open(&future).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    assert!(matches!(
        HistoryRepository::open(&future),
        Err(Error::IncompatibleSchema(_))
    ));
    let unrelated = dir.path().join("unrelated");
    let connection = Connection::open(&unrelated).unwrap();
    connection
        .execute("CREATE TABLE other (value TEXT)", [])
        .unwrap();
    drop(connection);
    assert!(matches!(
        HistoryRepository::open(&unrelated),
        Err(Error::IncompatibleSchema(_))
    ));
    let bad = dir.path().join("bad");
    fs::write(&bad, b"not a sqlite database").unwrap();
    assert!(matches!(
        HistoryRepository::open(&bad),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn invalid_rows_are_reported_without_panicking() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("db");
    let history = HistoryRepository::open(&path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO color_history VALUES ('not-a-uuid', 'invalid', 0, 0, 0, 255, 'picker')",
            [],
        )
        .unwrap();
    assert!(matches!(history.list(), Err(Error::Malformed(_))));
}

#[test]
fn concurrent_open_and_write_serializes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("db");
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut history = HistoryRepository::open(path).unwrap();
                for _ in 0..10 {
                    history.record(color(), CaptureSource::Picker).unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let history = HistoryRepository::open(path).unwrap();
    assert_eq!(history.list().unwrap().len(), 40);
}
