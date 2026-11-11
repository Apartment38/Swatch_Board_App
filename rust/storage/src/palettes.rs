use std::path::Path;

use rusqlite::{Connection, TransactionBehavior, params};
use uuid::Uuid;

use crate::{Error, HistoryRepository, Rgba8};

#[derive(Clone, Debug, PartialEq)]
pub struct SavedPalette {
    pub id: String,
    pub name: String,
    pub colors: Vec<Rgba8>,
    pub locked: Vec<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationRecord {
    pub id: String,
    pub description: String,
    pub palette: SavedPalette,
}

pub struct PaletteRepository {
    connection: Connection,
}

impl PaletteRepository {
    pub fn open_default() -> Result<Self, Error> {
        Ok(Self {
            connection: HistoryRepository::open_default()?.connection,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(Self {
            connection: HistoryRepository::open(path)?.connection,
        })
    }

    pub fn list(&self) -> Result<Vec<SavedPalette>, Error> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name FROM palettes ORDER BY rowid DESC")?;
        let mut rows = statement.query([])?;
        let mut palettes = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let (colors, locked) = read_colors(&self.connection, false, &id)?;
            palettes.push(SavedPalette {
                id,
                name,
                colors,
                locked,
            });
        }
        Ok(palettes)
    }

    pub fn save(
        &mut self,
        id: Option<&str>,
        name: &str,
        colors: &[Rgba8],
        locked: &[bool],
    ) -> Result<SavedPalette, Error> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 256 {
            return Err(Error::InvalidInput(
                "palette name must contain 1 to 256 characters".into(),
            ));
        }
        validate_colors(colors, locked)?;
        let existing = id.is_some();
        let id = id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if existing {
            if tx.execute(
                "UPDATE palettes SET name = ?1 WHERE id = ?2",
                params![name, id],
            )? == 0
            {
                return Err(Error::NotFound(format!("palette {id}")));
            }
            tx.execute("DELETE FROM palette_colors WHERE palette_id = ?1", [&id])?;
        } else {
            tx.execute(
                "INSERT INTO palettes (id, name) VALUES (?1, ?2)",
                params![id, name],
            )?;
        }
        insert_colors(&tx, false, &id, colors, locked)?;
        tx.commit()?;
        Ok(SavedPalette {
            id,
            name: name.to_owned(),
            colors: colors.to_vec(),
            locked: locked.to_vec(),
        })
    }

    pub fn delete(&mut self, id: &str) -> Result<(), Error> {
        if self
            .connection
            .execute("DELETE FROM palettes WHERE id = ?1", [id])?
            == 0
        {
            return Err(Error::NotFound(format!("palette {id}")));
        }
        Ok(())
    }

    pub fn generations(&self) -> Result<Vec<GenerationRecord>, Error> {
        let mut statement = self
            .connection
            .prepare("SELECT id, description FROM palette_generations ORDER BY rowid DESC")?;
        let mut rows = statement.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let description: String = row.get(1)?;
            let (colors, locked) = read_colors(&self.connection, true, &id)?;
            records.push(GenerationRecord {
                id,
                description,
                palette: SavedPalette {
                    id: String::new(),
                    name: String::new(),
                    colors,
                    locked,
                },
            });
        }
        Ok(records)
    }

    pub fn record_generation(
        &mut self,
        description: &str,
        colors: &[Rgba8],
        locked: &[bool],
    ) -> Result<(), Error> {
        validate_colors(colors, locked)?;
        let id = Uuid::new_v4().to_string();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO palette_generations (id, description) VALUES (?1, ?2)",
            params![id, description],
        )?;
        insert_colors(&tx, true, &id, colors, locked)?;
        tx.execute("DELETE FROM palette_generations WHERE rowid IN (SELECT rowid FROM palette_generations ORDER BY rowid DESC LIMIT -1 OFFSET 100)", [])?;
        tx.commit()?;
        Ok(())
    }
}

fn validate_colors(colors: &[Rgba8], locked: &[bool]) -> Result<(), Error> {
    if !(1..=64).contains(&colors.len()) || colors.len() != locked.len() {
        return Err(Error::InvalidInput(
            "palette must have 1 to 64 colors and one lock per color".into(),
        ));
    }
    Ok(())
}

fn insert_colors(
    connection: &Connection,
    generation: bool,
    id: &str,
    colors: &[Rgba8],
    locked: &[bool],
) -> Result<(), Error> {
    let sql = if generation {
        "INSERT INTO generation_colors (generation_id, position, red, green, blue, alpha, locked) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    } else {
        "INSERT INTO palette_colors (palette_id, position, red, green, blue, alpha, locked) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    };
    let mut statement = connection.prepare_cached(sql)?;
    for (position, (color, locked)) in colors.iter().zip(locked).enumerate() {
        statement.execute(params![
            id,
            position as i64,
            color.r,
            color.g,
            color.b,
            color.a,
            locked
        ])?;
    }
    Ok(())
}

fn read_colors(
    connection: &Connection,
    generation: bool,
    id: &str,
) -> Result<(Vec<Rgba8>, Vec<bool>), Error> {
    let sql = if generation {
        "SELECT red, green, blue, alpha, locked FROM generation_colors WHERE generation_id = ?1 ORDER BY position"
    } else {
        "SELECT red, green, blue, alpha, locked FROM palette_colors WHERE palette_id = ?1 ORDER BY position"
    };
    let mut statement = connection.prepare_cached(sql)?;
    let mut rows = statement.query([id])?;
    let mut colors = Vec::new();
    let mut locked = Vec::new();
    while let Some(row) = rows.next()? {
        colors.push(Rgba8 {
            r: row.get(0)?,
            g: row.get(1)?,
            b: row.get(2)?,
            a: row.get(3)?,
        });
        locked.push(row.get(4)?);
    }
    Ok((colors, locked))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CaptureSource;

    fn color(r: u8) -> Rgba8 {
        Rgba8 {
            r,
            g: 2,
            b: 3,
            a: 255,
        }
    }

    #[test]
    fn save_update_reopen_delete_and_generation_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("storage.db");
        let mut repo = PaletteRepository::open(&path).unwrap();
        let first = repo
            .save(None, "  First  ", &[color(1), color(4)], &[true, false])
            .unwrap();
        assert_eq!(first.name, "First");
        let second = repo.save(None, "Second", &[color(7)], &[false]).unwrap();
        let updated = repo
            .save(Some(&first.id), "Changed", &[color(9)], &[true])
            .unwrap();
        assert!(
            repo.save(Some("missing"), "Missing", &[color(1)], &[false])
                .is_err()
        );
        assert!(repo.save(Some(&first.id), "Bad", &[color(1)], &[]).is_err());
        for i in 0..101 {
            repo.record_generation(&i.to_string(), &[color(i as u8)], &[i % 2 == 0])
                .unwrap();
        }
        drop(repo);
        let mut repo = PaletteRepository::open(&path).unwrap();
        assert_eq!(repo.list().unwrap(), vec![second.clone(), updated]);
        let generations = repo.generations().unwrap();
        assert_eq!(generations.len(), 100);
        assert_eq!(generations[0].description, "100");
        assert_eq!(generations[0].palette.locked, vec![true]);
        assert_eq!(generations[99].description, "1");
        repo.delete(&second.id).unwrap();
        assert_eq!(repo.list().unwrap().len(), 1);
        assert_eq!(repo.generations().unwrap().len(), 100);
    }

    #[test]
    fn migration_preserves_history_and_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
            INSERT INTO schema_migrations VALUES (1, 'before');
            CREATE TABLE color_history (id TEXT NOT NULL UNIQUE, captured_at TEXT NOT NULL, red INTEGER NOT NULL, green INTEGER NOT NULL, blue INTEGER NOT NULL, alpha INTEGER NOT NULL, source TEXT NOT NULL);
            CREATE TABLE settings_meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
            INSERT INTO settings_meta VALUES ('theme', 'dark');
            INSERT INTO color_history VALUES ('00000000-0000-4000-8000-000000000001', '2025-01-01T00:00:00Z', 1, 2, 3, 255, 'picker');
            PRAGMA user_version = 1;").unwrap();
        drop(db);
        let history = HistoryRepository::open(&path).unwrap();
        assert_eq!(history.list().unwrap()[0].source, CaptureSource::Picker);
        assert_eq!(
            history
                .connection
                .query_row(
                    "SELECT value FROM settings_meta WHERE key='theme'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "dark"
        );
        assert_eq!(
            history
                .connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert!(
            PaletteRepository::open(&path)
                .unwrap()
                .list()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn failed_update_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = PaletteRepository::open(dir.path().join("rollback.db")).unwrap();
        let original = repo.save(None, "Original", &[color(1)], &[true]).unwrap();
        repo.connection.execute_batch("CREATE TRIGGER reject_colors BEFORE INSERT ON palette_colors BEGIN SELECT RAISE(ABORT, 'rejected'); END;").unwrap();
        assert!(
            repo.save(Some(&original.id), "Changed", &[color(2)], &[false])
                .is_err()
        );
        assert_eq!(repo.list().unwrap(), vec![original]);
    }
}
