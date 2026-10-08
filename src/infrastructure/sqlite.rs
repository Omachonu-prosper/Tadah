use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::application::AppError;
use crate::application::ports::{ListRepository, Result, Store, SubtaskRepository, TaskRepository};
use crate::domain::{Status, Subtask, Task, TodoList};

/// Ordered migrations; `PRAGMA user_version` records how many have run.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE lists (
        id         TEXT PRIMARY KEY,
        title      TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 100),
        position   INTEGER NOT NULL UNIQUE CHECK (position >= 0),
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
     );
     CREATE TABLE tasks (
        id           TEXT PRIMARY KEY,
        list_id      TEXT NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
        text         TEXT NOT NULL CHECK (length(trim(text)) BETWEEN 1 AND 500),
        status       TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'partial', 'done')),
        position     INTEGER NOT NULL CHECK (position >= 0),
        created_at   INTEGER NOT NULL,
        updated_at   INTEGER NOT NULL,
        completed_at INTEGER,
        UNIQUE (list_id, position),
        -- a task has a completion time exactly when it is done
        CHECK ((status = 'done') = (completed_at IS NOT NULL))
     );
     CREATE INDEX tasks_status_idx ON tasks (status);",
    // v2: subtasks (steps under a task)
    "CREATE TABLE subtasks (
        id         TEXT PRIMARY KEY,
        task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        text       TEXT NOT NULL CHECK (length(trim(text)) BETWEEN 1 AND 500),
        done       INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
        position   INTEGER NOT NULL CHECK (position >= 0),
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        UNIQUE (task_id, position)
     );",
    // v3: when a subtask was completed. SQLite can't add a CHECK with ALTER, so the table is
    // rebuilt to keep the "completed_at is set exactly when done" invariant in the database.
    "CREATE TABLE subtasks_new (
        id           TEXT PRIMARY KEY,
        task_id      TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        text         TEXT NOT NULL CHECK (length(trim(text)) BETWEEN 1 AND 500),
        done         INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
        position     INTEGER NOT NULL CHECK (position >= 0),
        created_at   INTEGER NOT NULL,
        updated_at   INTEGER NOT NULL,
        completed_at INTEGER,
        UNIQUE (task_id, position),
        CHECK ((done = 1) = (completed_at IS NOT NULL))
     );
     -- best available: a done subtask was last touched when it was checked
     INSERT INTO subtasks_new
        SELECT id, task_id, text, done, position, created_at, updated_at,
               CASE WHEN done = 1 THEN updated_at END
        FROM subtasks;
     DROP TABLE subtasks;
     ALTER TABLE subtasks_new RENAME TO subtasks;",
];

pub struct SqliteStore {
    conn: Connection,
}

fn storage(e: rusqlite::Error) -> AppError {
    AppError::Storage(e.to_string())
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| AppError::Storage(e.to_string()))?;
        }
        Self::init(Connection::open(path).map_err(storage)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory().map_err(storage)?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .map_err(storage)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let applied: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(storage)?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(applied as usize) {
            self.conn
                .execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;", i + 1))
                .map_err(storage)?;
        }
        Ok(())
    }
}

fn list_from_row(r: &Row) -> rusqlite::Result<TodoList> {
    Ok(TodoList {
        id: r.get("id")?,
        title: r.get("title")?,
        position: r.get("position")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

fn task_from_row(r: &Row) -> rusqlite::Result<Task> {
    let status: String = r.get("status")?;
    Ok(Task {
        id: r.get("id")?,
        list_id: r.get("list_id")?,
        text: r.get("text")?,
        status: Status::parse(&status).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown status {status:?}").into(),
            )
        })?,
        position: r.get("position")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        completed_at: r.get("completed_at")?,
    })
}

impl ListRepository for SqliteStore {
    fn lists(&self) -> Result<Vec<TodoList>> {
        let mut stmt = self.conn.prepare("SELECT * FROM lists ORDER BY position").map_err(storage)?;
        stmt.query_map([], list_from_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }

    fn list(&self, id: &str) -> Result<Option<TodoList>> {
        self.conn
            .query_row("SELECT * FROM lists WHERE id = ?1", [id], list_from_row)
            .optional()
            .map_err(storage)
    }

    fn next_list_position(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM lists", [], |r| r.get(0))
            .map_err(storage)
    }

    fn insert_list(&self, l: &TodoList) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO lists (id, title, position, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![l.id, l.title, l.position, l.created_at, l.updated_at],
            )
            .map(drop)
            .map_err(storage)
    }

    fn update_list(&self, l: &TodoList) -> Result<()> {
        self.conn
            .execute(
                "UPDATE lists SET title = ?2, updated_at = ?3 WHERE id = ?1",
                params![l.id, l.title, l.updated_at],
            )
            .map(drop)
            .map_err(storage)
    }

    fn delete_list(&self, id: &str) -> Result<()> {
        // Tasks go with it via ON DELETE CASCADE.
        self.conn.execute("DELETE FROM lists WHERE id = ?1", [id]).map(drop).map_err(storage)
    }
}

impl TaskRepository for SqliteStore {
    fn tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM tasks ORDER BY list_id, position")
            .map_err(storage)?;
        stmt.query_map([], task_from_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }

    fn task(&self, id: &str) -> Result<Option<Task>> {
        self.conn
            .query_row("SELECT * FROM tasks WHERE id = ?1", [id], task_from_row)
            .optional()
            .map_err(storage)
    }

    fn next_task_position(&self, list_id: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE list_id = ?1",
                [list_id],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    fn insert_task(&self, t: &Task) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO tasks (id, list_id, text, status, position, created_at, updated_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    t.id,
                    t.list_id,
                    t.text,
                    t.status.as_str(),
                    t.position,
                    t.created_at,
                    t.updated_at,
                    t.completed_at
                ],
            )
            .map(drop)
            .map_err(storage)
    }

    fn update_task(&self, t: &Task) -> Result<()> {
        self.conn
            .execute(
                "UPDATE tasks SET text = ?2, status = ?3, updated_at = ?4, completed_at = ?5 WHERE id = ?1",
                params![t.id, t.text, t.status.as_str(), t.updated_at, t.completed_at],
            )
            .map(drop)
            .map_err(storage)
    }

    fn delete_task(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?1", [id]).map(drop).map_err(storage)
    }
}

fn subtask_from_row(r: &Row) -> rusqlite::Result<Subtask> {
    Ok(Subtask {
        id: r.get("id")?,
        task_id: r.get("task_id")?,
        text: r.get("text")?,
        done: r.get("done")?,
        position: r.get("position")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        completed_at: r.get("completed_at")?,
    })
}

impl SubtaskRepository for SqliteStore {
    fn subtasks(&self) -> Result<Vec<Subtask>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM subtasks ORDER BY task_id, position")
            .map_err(storage)?;
        stmt.query_map([], subtask_from_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<_>>()
            .map_err(storage)
    }

    fn subtask(&self, id: &str) -> Result<Option<Subtask>> {
        self.conn
            .query_row("SELECT * FROM subtasks WHERE id = ?1", [id], subtask_from_row)
            .optional()
            .map_err(storage)
    }

    fn next_subtask_position(&self, task_id: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM subtasks WHERE task_id = ?1",
                [task_id],
                |r| r.get(0),
            )
            .map_err(storage)
    }

    fn insert_subtask(&self, s: &Subtask) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO subtasks (id, task_id, text, done, position, created_at, updated_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    s.id,
                    s.task_id,
                    s.text,
                    s.done,
                    s.position,
                    s.created_at,
                    s.updated_at,
                    s.completed_at
                ],
            )
            .map(drop)
            .map_err(storage)
    }

    fn update_subtask(&self, s: &Subtask) -> Result<()> {
        self.conn
            .execute(
                "UPDATE subtasks SET text = ?2, done = ?3, updated_at = ?4, completed_at = ?5 WHERE id = ?1",
                params![s.id, s.text, s.done, s.updated_at, s.completed_at],
            )
            .map(drop)
            .map_err(storage)
    }

    fn delete_subtask(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM subtasks WHERE id = ?1", [id]).map(drop).map_err(storage)
    }
}

impl Store for SqliteStore {
    fn transaction<T>(&self, work: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE").map_err(storage)?;
        match work(self) {
            Ok(v) => {
                self.conn.execute_batch("COMMIT").map_err(storage)?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a database exactly as schema v2 left it, then upgrades it.
    #[test]
    fn v3_backfills_subtask_completion_times() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.execute_batch(
            "PRAGMA user_version = 2;
             INSERT INTO lists VALUES ('l', 'L', 0, 1, 1);
             INSERT INTO tasks VALUES ('t', 'l', 'task', 'partial', 0, 1, 1, NULL);
             INSERT INTO subtasks VALUES ('done', 't', 'checked', 1, 0, 10, 55);
             INSERT INTO subtasks VALUES ('open', 't', 'unchecked', 0, 1, 10, 60);",
        )
        .unwrap();

        let store = SqliteStore::init(conn).unwrap();

        let version: i64 = store.conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(version as usize, MIGRATIONS.len());
        let subs = store.subtasks().unwrap();
        let by = |id: &str| subs.iter().find(|s| s.id == id).unwrap();
        assert_eq!(by("done").completed_at, Some(55)); // taken from updated_at
        assert_eq!(by("open").completed_at, None);
        assert_eq!(store.tasks().unwrap().len(), 1); // nothing else was lost
    }

    #[test]
    fn database_rejects_done_without_a_completion_time() {
        let store = SqliteStore::open_in_memory().unwrap();
        store.conn.execute_batch(
            "INSERT INTO lists VALUES ('l', 'L', 0, 1, 1);
             INSERT INTO tasks VALUES ('t', 'l', 'task', 'partial', 0, 1, 1, NULL);",
        ).unwrap();
        let bad = store.conn.execute(
            "INSERT INTO subtasks VALUES ('s', 't', 'x', 1, 0, 1, 1, NULL)",
            [],
        );
        assert!(bad.is_err());
    }
}
