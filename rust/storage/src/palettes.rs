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
    pub swatch_names: Vec<String>,
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
            let id: String = row.get(0).map_err(decode_malformed)?;
            let name: String = row.get(1).map_err(decode_malformed)?;
            let stored = read_colors(&self.connection, false, &id)?;
            let (swatch_positions, swatch_names) = read_swatch_names(&self.connection, &id)?;
            if !(1..=64).contains(&stored.colors.len())
                || stored.colors.len() != stored.locked.len()
                || stored.positions != swatch_positions
            {
                return Err(Error::Malformed(format!(
                    "palette {id} has invalid color, lock, or swatch name positions"
                )));
            }
            validate_loaded_swatch_names(&id, &swatch_names)?;
            palettes.push(SavedPalette {
                id,
                name,
                colors: stored.colors,
                locked: stored.locked,
                swatch_names,
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
        let mut names = match id {
            Some(id) => read_swatch_names(&self.connection, id)?.1,
            None => Vec::new(),
        };
        names.truncate(colors.len());
        while names.len() < colors.len() {
            let mut number = names.len() + 1;
            while names.iter().any(|name| name == &format!("Swatch {number}")) {
                number += 1;
            }
            names.push(format!("Swatch {number}"));
        }
        self.save_named(id, name, colors, locked, &names)
    }

    /// Save colors and their stable, unique swatch names in matching order.
    pub fn save_named(
        &mut self,
        id: Option<&str>,
        name: &str,
        colors: &[Rgba8],
        locked: &[bool],
        swatch_names: &[String],
    ) -> Result<SavedPalette, Error> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 256 {
            return Err(Error::InvalidInput(
                "palette name must contain 1 to 256 characters".into(),
            ));
        }
        validate_colors(colors, locked)?;
        let mut unique_names = std::collections::HashSet::new();
        if swatch_names.len() != colors.len()
            || swatch_names
                .iter()
                .any(|name| name.trim().is_empty() || !unique_names.insert(name))
        {
            return Err(Error::InvalidInput(
                "swatch names must be nonempty, unique, and match colors".into(),
            ));
        }
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
        {
            let mut statement = tx.prepare(
                "INSERT INTO palette_swatches (palette_id, position, name) VALUES (?1, ?2, ?3)",
            )?;
            for (position, swatch_name) in swatch_names.iter().enumerate() {
                statement.execute(params![id, position as i64, swatch_name])?;
            }
        }
        tx.commit()?;
        Ok(SavedPalette {
            id,
            name: name.to_owned(),
            colors: colors.to_vec(),
            locked: locked.to_vec(),
            swatch_names: swatch_names.to_vec(),
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
            let id: String = row.get(0).map_err(decode_malformed)?;
            let description: String = row.get(1).map_err(decode_malformed)?;
            let stored = read_colors(&self.connection, true, &id)?;
            if !(1..=64).contains(&stored.colors.len())
                || stored.colors.len() != stored.locked.len()
            {
                return Err(Error::Malformed(format!(
                    "generation {id} has invalid color or lock count"
                )));
            }
            let swatch_names = (1..=stored.colors.len())
                .map(|i| format!("Swatch {i}"))
                .collect();
            records.push(GenerationRecord {
                id,
                description,
                palette: SavedPalette {
                    id: String::new(),
                    name: String::new(),
                    colors: stored.colors,
                    locked: stored.locked,
                    swatch_names,
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

fn decode_malformed(error: rusqlite::Error) -> Error {
    Error::Malformed(error.to_string())
}

fn validate_loaded_swatch_names(id: &str, swatch_names: &[String]) -> Result<(), Error> {
    let mut unique_names = std::collections::HashSet::new();
    if swatch_names.iter().any(|name| {
        name.trim().is_empty() || name.chars().count() > 256 || !unique_names.insert(name)
    }) {
        return Err(Error::Malformed(format!(
            "palette {id} has invalid or duplicate swatch names"
        )));
    }
    Ok(())
}

fn read_swatch_names(connection: &Connection, id: &str) -> Result<(Vec<i64>, Vec<String>), Error> {
    let mut statement = connection.prepare(
        "SELECT position, name FROM palette_swatches WHERE palette_id = ?1 ORDER BY position",
    )?;
    let mut rows = statement.query([id])?;
    let mut positions = Vec::new();
    let mut names = Vec::new();
    while let Some(row) = rows.next()? {
        positions.push(row.get(0).map_err(decode_malformed)?);
        names.push(row.get(1).map_err(decode_malformed)?);
    }
    Ok((positions, names))
}

struct StoredColors {
    positions: Vec<i64>,
    colors: Vec<Rgba8>,
    locked: Vec<bool>,
}

fn read_colors(connection: &Connection, generation: bool, id: &str) -> Result<StoredColors, Error> {
    let sql = if generation {
        "SELECT position, red, green, blue, alpha, locked FROM generation_colors WHERE generation_id = ?1 ORDER BY position"
    } else {
        "SELECT position, red, green, blue, alpha, locked FROM palette_colors WHERE palette_id = ?1 ORDER BY position"
    };
    let mut statement = connection.prepare_cached(sql)?;
    let mut rows = statement.query([id])?;
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut locked = Vec::new();
    while let Some(row) = rows.next()? {
        positions.push(row.get(0).map_err(decode_malformed)?);
        colors.push(Rgba8 {
            r: row.get(1).map_err(decode_malformed)?,
            g: row.get(2).map_err(decode_malformed)?,
            b: row.get(3).map_err(decode_malformed)?,
            a: row.get(4).map_err(decode_malformed)?,
        });
        locked.push(row.get(5).map_err(decode_malformed)?);
    }
    Ok(StoredColors {
        positions,
        colors,
        locked,
    })
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

    #[test]
    fn list_rejects_corrupt_palette_counts_and_missing_swatch_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.db");
        let mut repo = PaletteRepository::open(&path).unwrap();
        let saved = repo
            .save(None, "Two", &[color(1), color(2)], &[false, true])
            .unwrap();

        // Missing palette_swatches row causes swatch_names.len() != colors.len().
        repo.connection
            .execute(
                "DELETE FROM palette_swatches WHERE palette_id = ?1 AND position = 1",
                [&saved.id],
            )
            .unwrap();
        assert!(matches!(repo.list(), Err(Error::Malformed(_))));

        // Restore swatch name, then delete all colors so colors.len() == 0.
        repo.connection
            .execute(
                "DELETE FROM palette_colors WHERE palette_id = ?1",
                [&saved.id],
            )
            .unwrap();
        assert!(matches!(repo.list(), Err(Error::Malformed(_))));

        // Insert 65 colors and 65 swatch names (> 64 limit).
        for pos in 0..65_i64 {
            repo.connection
                .execute(
                    "INSERT INTO palette_colors (palette_id, position, red, green, blue, alpha, locked) VALUES (?1, ?2, 1, 2, 3, 255, 0)",
                    params![&saved.id, pos],
                )
                .unwrap();
            repo.connection
                .execute(
                    "INSERT INTO palette_swatches (palette_id, position, name) VALUES (?1, ?2, ?3)",
                    params![&saved.id, pos, format!("Swatch {}", pos + 1)],
                )
                .unwrap();
        }
        assert!(matches!(repo.list(), Err(Error::Malformed(_))));
    }

    #[test]
    fn list_and_generations_reject_corrupt_types_mismatched_positions_and_invalid_swatch_names() {
        // 1. REAL red channel in palette_colors -> Error::Malformed
        {
            let dir = tempfile::tempdir().unwrap();
            let mut repo = PaletteRepository::open(dir.path().join("real_red.db")).unwrap();
            let saved = repo.save(None, "Palette", &[color(1)], &[false]).unwrap();
            repo.connection
                .execute(
                    "UPDATE palette_colors SET red = 1.5 WHERE palette_id = ?1 AND position = 0",
                    [&saved.id],
                )
                .unwrap();
            assert!(matches!(repo.list(), Err(Error::Malformed(_))));
        }

        // 2. REAL red channel and BLOB description in generations -> Error::Malformed
        {
            let dir = tempfile::tempdir().unwrap();
            let mut repo = PaletteRepository::open(dir.path().join("gen_corrupt.db")).unwrap();
            repo.record_generation("desc", &[color(1)], &[false])
                .unwrap();
            let gen_id = repo.generations().unwrap()[0].id.clone();
            repo.connection
                .execute(
                    "UPDATE generation_colors SET red = 2.5 WHERE generation_id = ?1 AND position = 0",
                    [&gen_id],
                )
                .unwrap();
            assert!(matches!(repo.generations(), Err(Error::Malformed(_))));

            repo.connection
                .execute(
                    "UPDATE generation_colors SET red = 2 WHERE generation_id = ?1 AND position = 0",
                    [&gen_id],
                )
                .unwrap();
            repo.connection
                .execute(
                    "UPDATE palette_generations SET description = X'deadbeef' WHERE id = ?1",
                    [&gen_id],
                )
                .unwrap();
            assert!(matches!(repo.generations(), Err(Error::Malformed(_))));
        }

        // 3. BLOB swatch name -> Error::Malformed
        {
            let dir = tempfile::tempdir().unwrap();
            let mut repo = PaletteRepository::open(dir.path().join("blob_swatch.db")).unwrap();
            let saved = repo.save(None, "Palette", &[color(1)], &[false]).unwrap();
            repo.connection
                .execute(
                    "UPDATE palette_swatches SET name = X'6162' WHERE palette_id = ?1 AND position = 0",
                    [&saved.id],
                )
                .unwrap();
            assert!(matches!(repo.list(), Err(Error::Malformed(_))));
        }

        // 4. Mismatched palette_swatches positions ([0, 1] vs [1, 99]) -> Error::Malformed
        {
            let dir = tempfile::tempdir().unwrap();
            let mut repo = PaletteRepository::open(dir.path().join("mismatch_pos.db")).unwrap();
            let saved = repo
                .save(None, "Two", &[color(1), color(2)], &[false, true])
                .unwrap();
            repo.connection
                .pragma_update(None, "foreign_keys", "OFF")
                .unwrap();
            repo.connection
                .execute(
                    "UPDATE palette_swatches SET position = 99 WHERE palette_id = ?1 AND position = 0",
                    [&saved.id],
                )
                .unwrap();
            repo.connection
                .pragma_update(None, "foreign_keys", "ON")
                .unwrap();
            assert!(matches!(repo.list(), Err(Error::Malformed(_))));
        }

        // 5. Blank, overly long, or duplicate swatch names -> Error::Malformed
        {
            let dir = tempfile::tempdir().unwrap();
            let mut repo = PaletteRepository::open(dir.path().join("bad_names.db")).unwrap();
            let saved = repo
                .save(None, "Names", &[color(1), color(2)], &[false, false])
                .unwrap();
            repo.connection
                .execute(
                    "UPDATE palette_swatches SET name = '   ' WHERE palette_id = ?1 AND position = 0",
                    [&saved.id],
                )
                .unwrap();
            assert!(matches!(repo.list(), Err(Error::Malformed(_))));

            let too_long = "a".repeat(257);
            repo.connection
                .execute(
                    "UPDATE palette_swatches SET name = ?2 WHERE palette_id = ?1 AND position = 0",
                    params![&saved.id, &too_long],
                )
                .unwrap();
            assert!(matches!(repo.list(), Err(Error::Malformed(_))));

            // Rebuild palette_swatches without UNIQUE(palette_id, name) to simulate corrupt duplicates.
            repo.connection
                .execute_batch(
                    "DROP TABLE palette_swatches;
                     CREATE TABLE palette_swatches (palette_id TEXT NOT NULL, position INTEGER NOT NULL, name TEXT NOT NULL);",
                )
                .unwrap();
            repo.connection
                .execute(
                    "INSERT INTO palette_swatches (palette_id, position, name) VALUES (?1, 0, 'Dup'), (?1, 1, 'Dup')",
                    [&saved.id],
                )
                .unwrap();
            assert!(matches!(repo.list(), Err(Error::Malformed(_))));
        }
    }
}
