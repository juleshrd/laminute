use rusqlite::Connection;

use crate::error::AppResult;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("./migrations");
}

pub fn run_migrations(conn: &mut Connection) -> AppResult<()> {
    embedded::migrations::runner()
        .run(conn)
        .map(|_| ())
        .map_err(Into::into)
}

#[cfg(test)]
fn migration_count(conn: &Connection) -> AppResult<usize> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM refinery_schema_history", [], |row| {
        row.get(0)
    })?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn migrations_apply_cleanly_on_empty_database() {
        let conn = open_in_memory().expect("connexion mémoire");
        assert_eq!(migration_count(&conn).unwrap(), 3);
    }

    #[test]
    fn migrations_are_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        run_migrations(&mut conn).expect("première passe");
        run_migrations(&mut conn).expect("deuxième passe");

        assert_eq!(migration_count(&conn).unwrap(), 3);
    }

    #[test]
    fn schema_contains_expected_tables() {
        let conn = open_in_memory().expect("connexion mémoire");

        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table'
                   AND name NOT LIKE 'sqlite_%'
                   AND name NOT LIKE 'meetings_fts%'
                 ORDER BY name",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(
            tables,
            vec![
                "actions",
                "ai_jobs",
                "ai_providers",
                "audio_files",
                "meetings",
                "refinery_schema_history",
                "settings",
                "summaries",
                "transcriptions",
            ]
        );

        let fts_present: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE name = 'meetings_fts' AND type IN ('table', 'virtual')",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(fts_present, "meetings_fts virtual table must exist");
    }

    #[test]
    fn ai_providers_has_no_api_key_column() {
        let conn = open_in_memory().expect("connexion mémoire");

        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(ai_providers)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(!columns
            .iter()
            .any(|c| c.contains("api_key") || c.contains("secret")));
        assert!(columns.contains(&"credential_key_id".to_string()));
    }
}
