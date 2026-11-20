use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{Error, HistoryRepository};

/// A named project. The versioned payload is stored verbatim and interpreted by Board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTheme {
    pub id: String,
    pub name: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub palette_id: Option<String>,
    pub scene: String,
    pub payload: String,
    pub dirty: bool,
}

pub struct ThemeRepository {
    connection: Connection,
}

impl ThemeRepository {
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

    pub fn list(&self) -> Result<Vec<SavedTheme>, Error> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, plugin_id, plugin_version, palette_id, scene, payload, dirty FROM theme_projects ORDER BY rowid DESC",
        )?;
        let rows = statement.query_map([], decode_theme)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn load(&self, id: &str) -> Result<Option<SavedTheme>, Error> {
        Ok(self.connection.query_row(
            "SELECT id, name, plugin_id, plugin_version, palette_id, scene, payload, dirty FROM theme_projects WHERE id = ?1",
            [id], decode_theme,
        ).optional()?)
    }

    /// Insert or update a project atomically, returning its existing or generated ID.
    pub fn save(&mut self, project: &SavedTheme) -> Result<String, Error> {
        let valid: bool =
            self.connection
                .query_row("SELECT json_valid(?1)", [&project.payload], |row| {
                    row.get(0)
                })?;
        if !valid {
            return Err(Error::InvalidInput(
                "theme project payload must be valid JSON".into(),
            ));
        }
        let id = if project.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            project.id.clone()
        };
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT INTO theme_projects (id, name, plugin_id, plugin_version, palette_id, scene, payload, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, plugin_id=excluded.plugin_id,
               plugin_version=excluded.plugin_version, palette_id=excluded.palette_id,
               scene=excluded.scene, payload=excluded.payload, dirty=excluded.dirty",
            params![id, project.name, project.plugin_id, project.plugin_version, project.palette_id,
                project.scene, project.payload, project.dirty],
        )?;
        tx.execute("DELETE FROM theme_assignments WHERE project_id = ?1", [&id])?;
        tx.execute(
            "INSERT OR REPLACE INTO theme_assignments (project_id, role_id, assignment_json)
             SELECT ?1, key,
               CASE type
                 WHEN 'null' THEN 'null'
                 WHEN 'true' THEN 'true'
                 WHEN 'false' THEN 'false'
                 WHEN 'text' THEN json_quote(value)
                 WHEN 'integer' THEN json_quote(value)
                 WHEN 'real' THEN json_quote(value)
                 ELSE json(value)
               END
             FROM json_each(?2, '$.assignments')
             WHERE json_valid(?2) AND json_type(?2, '$.assignments') = 'object'",
            params![id, project.payload],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// The recoverable working draft is independent of named project records.
    pub fn save_draft(&mut self, payload: &str) -> Result<(), Error> {
        self.connection.execute(
            "INSERT INTO theme_draft (id, payload) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",
            [payload],
        )?;
        Ok(())
    }

    pub fn load_draft(&self) -> Result<Option<String>, Error> {
        Ok(self
            .connection
            .query_row("SELECT payload FROM theme_draft WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()?)
    }
}

fn decode_theme(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedTheme> {
    Ok(SavedTheme {
        id: row.get(0)?,
        name: row.get(1)?,
        plugin_id: row.get(2)?,
        plugin_version: row.get(3)?,
        palette_id: row.get(4)?,
        scene: row.get(5)?,
        payload: row.get(6)?,
        dirty: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_and_draft_roundtrip_independently() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        let mut repo = ThemeRepository::open(&path).unwrap();
        assert_eq!(repo.load_draft().unwrap(), None);
        assert_eq!(repo.load("missing").unwrap(), None);
        let mut project = SavedTheme {
            id: String::new(),
            name: "Example".into(),
            plugin_id: "terminal".into(),
            plugin_version: "1.0".into(),
            palette_id: Some("palette".into()),
            scene: "menu".into(),
            payload: "{ \"version\": 1, \"unknown\": [1, 2] }\n".into(),
            dirty: true,
        };
        project.id = repo.save(&project).unwrap();
        assert!(Uuid::parse_str(&project.id).is_ok());
        repo.save_draft("draft one").unwrap();
        repo.save_draft("draft two\n").unwrap();
        drop(repo);
        let mut repo = ThemeRepository::open(&path).unwrap();
        assert_eq!(repo.load(&project.id).unwrap(), Some(project.clone()));
        assert_eq!(repo.list().unwrap(), vec![project.clone()]);
        assert_eq!(repo.load_draft().unwrap().as_deref(), Some("draft two\n"));
        project.palette_id = None;
        project.dirty = false;
        project.payload = "{\"version\":2}".into();
        assert_eq!(repo.save(&project).unwrap(), project.id);
        assert_eq!(repo.load(&project.id).unwrap(), Some(project));
        assert_eq!(repo.load_draft().unwrap().as_deref(), Some("draft two\n"));
    }

    #[test]
    fn assignment_index_updates_atomically_with_project_payload() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = ThemeRepository::open(dir.path().join("themes.db")).unwrap();
        let mut project = SavedTheme {
            id: String::new(),
            name: "Indexed project".into(),
            plugin_id: "fixture".into(),
            plugin_version: "1".into(),
            palette_id: None,
            scene: "main".into(),
            payload: r##"{"assignments":{"background":{"kind":"Override","value":"#123456"}}}"##
                .into(),
            dirty: false,
        };
        project.id = repo.save(&project).unwrap();
        let indexed: String = repo.connection.query_row(
            "SELECT assignment_json FROM theme_assignments WHERE project_id=?1 AND role_id='background'",
            [&project.id], |row| row.get(0),
        ).unwrap();
        assert_eq!(indexed, r##"{"kind":"Override","value":"#123456"}"##);
        let saved = project.clone();
        project.name = "Must roll back".into();
        project.payload = "not JSON".into();
        assert!(repo.save(&project).is_err());
        assert_eq!(repo.load(&saved.id).unwrap(), Some(saved.clone()));
        let remaining: i64 = repo
            .connection
            .query_row(
                "SELECT count(*) FROM theme_assignments WHERE project_id=?1",
                [&saved.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
        project = saved;
        project.payload = r#"{"assignments":{"accent":{"kind":"Default"}}}"#.into();
        repo.save(&project).unwrap();
        let role: String = repo
            .connection
            .query_row(
                "SELECT role_id FROM theme_assignments WHERE project_id=?1",
                [&project.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(role, "accent");
    }

    #[test]
    fn assignment_index_preserves_valid_json_for_all_value_types() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = ThemeRepository::open(dir.path().join("types.db")).unwrap();
        let mut project = SavedTheme {
            id: String::new(),
            name: "All JSON types".into(),
            plugin_id: "fixture".into(),
            plugin_version: "1".into(),
            palette_id: None,
            scene: "main".into(),
            payload: r##"{"assignments":{"n":null,"t":true,"f":false,"i":42,"neg":-7,"r":3.5,"s":"#123456","q":"a \"quote\"\nline","e":"","arr":[1,"a",null,true],"obj":{"kind":"Override","value":"#123456"}}}"##
                .into(),
            dirty: false,
        };
        project.id = repo.save(&project).unwrap();

        let expected = [
            ("n", "null"),
            ("t", "true"),
            ("f", "false"),
            ("i", "42"),
            ("neg", "-7"),
            ("r", "3.5"),
            ("s", r##""#123456""##),
            ("q", r#""a \"quote\"\nline""#),
            ("e", r#""""#),
            ("arr", r#"[1,"a",null,true]"#),
            ("obj", r##"{"kind":"Override","value":"#123456"}"##),
        ];
        for (role, expected_json) in expected {
            let (stored, valid): (String, bool) = repo
                .connection
                .query_row(
                    "SELECT assignment_json, json_valid(assignment_json) FROM theme_assignments WHERE project_id=?1 AND role_id=?2",
                    params![&project.id, role],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert!(
                valid,
                "stored assignment_json for {role} is not valid JSON: {stored}"
            );
            assert_eq!(stored, expected_json, "mismatch for role {role}");
        }
    }

    #[test]
    fn migration_from_v2_preserves_palette_and_initializes_names() {
        use crate::{PaletteRepository, Rgba8};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v2.db");
        let mut palettes = PaletteRepository::open(&path).unwrap();
        let colors = [
            Rgba8 {
                r: 1,
                g: 2,
                b: 3,
                a: 255,
            },
            Rgba8 {
                r: 4,
                g: 5,
                b: 6,
                a: 255,
            },
        ];
        let palette = palettes
            .save(None, "Keep", &colors, &[true, false])
            .unwrap();
        drop(palettes);
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "DROP TABLE theme_assignments; DROP TABLE theme_projects;
            DROP TABLE theme_draft; DROP TABLE palette_swatches;
            DELETE FROM schema_migrations WHERE version=3; PRAGMA user_version=2;",
        )
        .unwrap();
        drop(db);
        let mut palettes = PaletteRepository::open(&path).unwrap();
        assert_eq!(palettes.list().unwrap(), vec![palette.clone()]);
        let names = vec!["Accent".to_string(), "Background".to_string()];
        let updated = palettes
            .save_named(Some(&palette.id), "Keep", &colors, &[true, false], &names)
            .unwrap();
        drop(palettes);
        assert_eq!(
            PaletteRepository::open(&path).unwrap().list().unwrap(),
            vec![updated]
        );
        assert!(
            ThemeRepository::open(&path)
                .unwrap()
                .list()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn failed_v3_migration_rolls_back_without_losing_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        drop(HistoryRepository::open(&path).unwrap());
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "DROP TABLE theme_assignments; DROP TABLE theme_draft; DROP TABLE palette_swatches;
            DELETE FROM schema_migrations WHERE version=3; PRAGMA user_version=2;
            INSERT INTO settings_meta VALUES ('keep', 'value');",
        )
        .unwrap();
        drop(db);
        assert!(ThemeRepository::open(&path).is_err());
        let db = Connection::open(&path).unwrap();
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT value FROM settings_meta WHERE key='keep'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "value"
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM schema_migrations WHERE version=3",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn duplicate_assignment_keys_in_json_keep_last_value() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = ThemeRepository::open(dir.path().join("dup_keys.db")).unwrap();
        let mut project = SavedTheme {
            id: String::new(),
            name: "Duplicate keys".into(),
            plugin_id: "fixture".into(),
            plugin_version: "1".into(),
            palette_id: None,
            scene: "main".into(),
            payload: r##"{"assignments":{"bg":"#111111","bg":"#222222"}}"##.into(),
            dirty: false,
        };
        project.id = repo.save(&project).unwrap();
        let indexed: String = repo
            .connection
            .query_row(
                "SELECT assignment_json FROM theme_assignments WHERE project_id=?1 AND role_id='bg'",
                [&project.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexed, r##""#222222""##);
    }
}
