//! SQLite-backed capture history shared by desktop processes.
//!
//! SQLite is bundled for a consistent set of features. Each repository owns a
//! connection; separate processes may open the same database independently.

use std::env;
use std::fmt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, ErrorCode, OptionalExtension, TransactionBehavior, params};
pub use swatch_board_picker_core::Rgba8;
use uuid::Uuid;

mod palettes;
pub use palettes::{GenerationRecord, PaletteRepository, SavedPalette};

pub const DEFAULT_RETENTION: usize = 100;
const SCHEMA_VERSION: i64 = 2;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum Error {
    /// Neither XDG_DATA_HOME nor HOME supplied a usable absolute directory.
    DataDirectoryUnavailable,
    Io(std::io::Error),
    /// Another connection held a conflicting lock longer than the busy timeout.
    Busy(rusqlite::Error),
    /// The database is corrupt, not SQLite, or does not match our schema.
    Malformed(String),
    /// A newer or otherwise unrecognized schema must not be modified.
    IncompatibleSchema(String),
    InvalidInput(String),
    NotFound(String),
    Database(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataDirectoryUnavailable => write!(f, "no absolute XDG data directory or HOME"),
            Self::Io(e) => write!(f, "storage I/O error: {e}"),
            Self::Busy(e) => write!(f, "storage database is busy: {e}"),
            Self::Malformed(e) => write!(f, "malformed storage database: {e}"),
            Self::IncompatibleSchema(e) => write!(f, "incompatible storage schema: {e}"),
            Self::InvalidInput(e) => write!(f, "invalid input: {e}"),
            Self::NotFound(e) => write!(f, "not found: {e}"),
            Self::Database(e) => write!(f, "storage database error: {e}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        match value.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Self::Busy(value),
            Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => {
                Self::Malformed(value.to_string())
            }
            _ => Self::Database(value),
        }
    }
}

/// Resolve the library path according to the XDG Base Directory specification.
/// Relative and empty environment values are ignored, never shell-expanded.
pub fn database_path() -> Result<PathBuf, Error> {
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    resolve_database_path(data.as_deref(), home.as_deref())
}

fn resolve_database_path(data: Option<&Path>, home: Option<&Path>) -> Result<PathBuf, Error> {
    let base = data
        .filter(|p| p.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| {
            home.filter(|p| p.is_absolute())
                .map(|p| p.join(".local/share"))
        })
        .ok_or(Error::DataDirectoryUnavailable)?;
    Ok(base.join("swatch-board/library.sqlite3"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureSource {
    Picker,
    Board,
}
impl CaptureSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Picker => "picker",
            Self::Board => "board",
        }
    }
    fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "picker" => Ok(Self::Picker),
            "board" => Ok(Self::Board),
            _ => Err(Error::Malformed(format!(
                "unknown capture source {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capture {
    pub id: Uuid,
    pub captured_at: DateTime<Utc>,
    pub color: Rgba8,
    pub source: CaptureSource,
}

pub struct HistoryRepository {
    connection: Connection,
    retention: usize,
}

impl HistoryRepository {
    /// Open the standard XDG database, creating its parent directory as needed.
    pub fn open_default() -> Result<Self, Error> {
        Self::open(database_path()?)
    }

    /// Open or create a database at an explicit path. Parent directories are created.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true).mode(0o700).create(parent)?;
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        Self::from_connection(Connection::open(path)?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, Error> {
        connection.busy_timeout(BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        // Check/migrate before setting persistent WAL mode: incompatible files must
        // not be modified. The immediate transaction serializes concurrent startup.
        migrate(&mut connection)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self {
            connection,
            retention: DEFAULT_RETENTION,
        })
    }

    /// Set the maximum rows kept after each capture. Zero disables retention of rows.
    pub fn set_retention(&mut self, retention: usize) {
        self.retention = retention;
    }

    /// Atomically insert a distinct capture and prune the oldest rows.
    pub fn record(&mut self, color: Rgba8, source: CaptureSource) -> Result<Capture, Error> {
        let capture = Capture {
            id: Uuid::new_v4(),
            captured_at: Utc::now(),
            color,
            source,
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO color_history (id, captured_at, red, green, blue, alpha, source) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![capture.id.to_string(), capture.captured_at.to_rfc3339_opts(SecondsFormat::Nanos, true), color.r, color.g, color.b, color.a, source.as_str()],
        )?;
        // rowid represents commit/insertion order, even if wall clock moves backwards.
        let keep = i64::try_from(self.retention).unwrap_or(i64::MAX);
        tx.execute("DELETE FROM color_history WHERE rowid IN (SELECT rowid FROM color_history ORDER BY rowid DESC LIMIT -1 OFFSET ?1)", [keep])?;
        tx.commit()?;
        Ok(capture)
    }

    /// Read newest captures first, without modifying or reordering history.
    pub fn list(&self) -> Result<Vec<Capture>, Error> {
        let mut statement = self.connection.prepare("SELECT id, captured_at, red, green, blue, alpha, source FROM color_history ORDER BY rowid DESC")?;
        let mut rows = statement.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            result.push(decode_capture(row)?);
        }
        Ok(result)
    }

    pub fn last(&self) -> Result<Option<Capture>, Error> {
        let mut statement = self.connection.prepare("SELECT id, captured_at, red, green, blue, alpha, source FROM color_history ORDER BY rowid DESC LIMIT 1")?;
        let mut rows = statement.query([])?;
        rows.next()?.map(decode_capture).transpose()
    }
}

fn decode_capture(row: &rusqlite::Row<'_>) -> Result<Capture, Error> {
    let decode = |e: rusqlite::Error| Error::Malformed(e.to_string());
    let id: String = row.get(0).map_err(decode)?;
    let timestamp: String = row.get(1).map_err(decode)?;
    let source: String = row.get(6).map_err(decode)?;
    Ok(Capture {
        id: Uuid::parse_str(&id)
            .map_err(|e| Error::Malformed(format!("invalid capture UUID: {e}")))?,
        captured_at: DateTime::parse_from_rfc3339(&timestamp)
            .map_err(|e| Error::Malformed(format!("invalid capture timestamp: {e}")))?
            .with_timezone(&Utc),
        color: Rgba8 {
            r: row.get(2).map_err(decode)?,
            g: row.get(3).map_err(decode)?,
            b: row.get(4).map_err(decode)?,
            a: row.get(5).map_err(decode)?,
        },
        source: CaptureSource::parse(&source)?,
    })
}

fn migrate(connection: &mut Connection) -> Result<(), Error> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if !(0..=SCHEMA_VERSION).contains(&version) {
        return Err(Error::IncompatibleSchema(format!(
            "unsupported version {version}"
        )));
    }
    if version == 0 {
        let existing: Option<String> = tx.query_row(
            "SELECT name FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' LIMIT 1", [], |row| row.get(0)
        ).optional()?;
        if let Some(name) = existing {
            return Err(Error::IncompatibleSchema(format!(
                "unversioned database contains {name:?}"
            )));
        }
        tx.execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
            CREATE TABLE color_history (id TEXT NOT NULL UNIQUE, captured_at TEXT NOT NULL,
                red INTEGER NOT NULL CHECK(red BETWEEN 0 AND 255), green INTEGER NOT NULL CHECK(green BETWEEN 0 AND 255),
                blue INTEGER NOT NULL CHECK(blue BETWEEN 0 AND 255), alpha INTEGER NOT NULL CHECK(alpha BETWEEN 0 AND 255),
                source TEXT NOT NULL CHECK(source IN ('picker', 'board')));
            CREATE INDEX color_history_captured_at ON color_history(captured_at);
            CREATE TABLE settings_meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
            INSERT INTO schema_migrations (version, applied_at) VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
            PRAGMA user_version = 1;")?;
    }
    // Validate required tables/columns and migration marker before handing out a
    // connection; an unrelated database claiming our version is not compatible.
    let valid = tx
        .query_row(
            "SELECT count(*) FROM schema_migrations WHERE version = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| Error::IncompatibleSchema(format!("missing migration metadata: {e}")))?;
    if valid != 1 {
        return Err(Error::IncompatibleSchema("missing migration 1".into()));
    }
    tx.prepare(
        "SELECT id, captured_at, red, green, blue, alpha, source FROM color_history LIMIT 0",
    )
    .map_err(|e| Error::IncompatibleSchema(format!("invalid history schema: {e}")))?;
    tx.prepare("SELECT key, value FROM settings_meta LIMIT 0")
        .map_err(|e| Error::IncompatibleSchema(format!("invalid settings schema: {e}")))?;
    if version < 2 {
        tx.execute_batch("CREATE TABLE palettes (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL);
            CREATE TABLE palette_colors (palette_id TEXT NOT NULL REFERENCES palettes(id) ON DELETE CASCADE,
                position INTEGER NOT NULL, red INTEGER NOT NULL CHECK(red BETWEEN 0 AND 255),
                green INTEGER NOT NULL CHECK(green BETWEEN 0 AND 255), blue INTEGER NOT NULL CHECK(blue BETWEEN 0 AND 255),
                alpha INTEGER NOT NULL CHECK(alpha BETWEEN 0 AND 255), locked INTEGER NOT NULL CHECK(locked IN (0, 1)),
                PRIMARY KEY (palette_id, position));
            CREATE TABLE palette_generations (id TEXT PRIMARY KEY NOT NULL, description TEXT NOT NULL);
            CREATE TABLE generation_colors (generation_id TEXT NOT NULL REFERENCES palette_generations(id) ON DELETE CASCADE,
                position INTEGER NOT NULL, red INTEGER NOT NULL CHECK(red BETWEEN 0 AND 255),
                green INTEGER NOT NULL CHECK(green BETWEEN 0 AND 255), blue INTEGER NOT NULL CHECK(blue BETWEEN 0 AND 255),
                alpha INTEGER NOT NULL CHECK(alpha BETWEEN 0 AND 255), locked INTEGER NOT NULL CHECK(locked IN (0, 1)),
                PRIMARY KEY (generation_id, position));
            INSERT INTO schema_migrations (version, applied_at) VALUES (2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
            PRAGMA user_version = 2;")?;
    }
    let valid: i64 = tx
        .query_row(
            "SELECT count(*) FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get(0),
        )
        .map_err(|e| Error::IncompatibleSchema(format!("missing migration metadata: {e}")))?;
    if valid != 1 {
        return Err(Error::IncompatibleSchema("missing migration 2".into()));
    }
    for sql in [
        "SELECT id, name FROM palettes LIMIT 0",
        "SELECT palette_id, position, red, green, blue, alpha, locked FROM palette_colors LIMIT 0",
        "SELECT id, description FROM palette_generations LIMIT 0",
        "SELECT generation_id, position, red, green, blue, alpha, locked FROM generation_colors LIMIT 0",
    ] {
        tx.prepare(sql)
            .map_err(|e| Error::IncompatibleSchema(format!("invalid palette schema: {e}")))?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_paths_require_absolute_values_and_fall_back_to_home() {
        assert_eq!(
            resolve_database_path(Some(Path::new("/data")), Some(Path::new("/home/alice")))
                .unwrap(),
            PathBuf::from("/data/swatch-board/library.sqlite3")
        );
        assert_eq!(
            resolve_database_path(Some(Path::new("relative")), Some(Path::new("/home/alice")))
                .unwrap(),
            PathBuf::from("/home/alice/.local/share/swatch-board/library.sqlite3")
        );
        assert!(matches!(
            resolve_database_path(None, Some(Path::new("~"))),
            Err(Error::DataDirectoryUnavailable)
        ));
    }

    #[test]
    fn connection_enables_foreign_keys_and_busy_timeout() {
        let history =
            HistoryRepository::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let foreign_keys: i64 = history
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        let timeout: i64 = history
            .connection
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert_eq!(timeout, BUSY_TIMEOUT.as_millis() as i64);
    }
}
