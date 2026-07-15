use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::app::{new_uuid, App, GroupingMode, Priority, SubTask, Task, View};

pub fn open_and_init(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tasks (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            sort_order      INTEGER NOT NULL DEFAULT 0,
            uuid            TEXT,
            name            TEXT NOT NULL,
            notes           TEXT,
            project         TEXT,
            priority        INTEGER NOT NULL DEFAULT 1,
            due_date        TEXT,
            due_notified    INTEGER NOT NULL DEFAULT 0,
            completed       INTEGER NOT NULL DEFAULT 0,
            creation_date   TEXT NOT NULL,
            completion_date TEXT
        );
        CREATE TABLE IF NOT EXISTS subtasks (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            task_uuid       TEXT NOT NULL,
            sort_order      INTEGER NOT NULL DEFAULT 0,
            name            TEXT NOT NULL,
            done            INTEGER NOT NULL DEFAULT 0,
            creation_date   TEXT NOT NULL,
            completion_date TEXT
        );
        CREATE TABLE IF NOT EXISTS app_state (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    // Migrate databases created before the uuid column existed. Ignore the
    // error raised when the column is already present.
    let _ = conn.execute("ALTER TABLE tasks ADD COLUMN uuid TEXT", []);
    Ok(())
}

fn get_state(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .ok()
}

fn priority_from_int(i: i64) -> Priority {
    match i {
        0 => Priority::Low,
        2 => Priority::High,
        _ => Priority::Medium,
    }
}

fn priority_to_int(p: Priority) -> i64 {
    match p {
        Priority::Low => 0,
        Priority::Medium => 1,
        Priority::High => 2,
    }
}

pub struct LoadedState {
    pub tasks: Vec<Task>,
    pub current_view: View,
    pub grouping_mode: GroupingMode,
    pub active_task_index: Option<usize>,
}

pub fn load_from(conn: &Connection) -> LoadedState {
    let tasks = load_tasks(conn).unwrap_or_default();
    let current_view = get_state(conn, "current_view")
        .map(|s| match s.as_str() {
            "Statistics" => View::Statistics,
            // "Dashboard" is a legacy value (Phase 3 merged it into TaskList).
            _ => View::TaskList,
        })
        .unwrap_or_default();
    // Prefer the new `grouping_mode` key; fall back to the legacy `sort_mode`
    // written by pre-Phase-3 builds so upgrades are lossless.
    let grouping_mode = get_state(conn, "grouping_mode")
        .and_then(|s| match s.as_str() {
            "Smart" => Some(GroupingMode::Smart),
            "Project" => Some(GroupingMode::Project),
            "Priority" => Some(GroupingMode::Priority),
            "Manual" => Some(GroupingMode::Manual),
            _ => None,
        })
        .or_else(|| {
            get_state(conn, "sort_mode").map(|s| match s.as_str() {
                "Priority" => GroupingMode::Priority,
                // Legacy "DueDate" mapped to Smart, which sorts dated tasks by due date.
                "DueDate" => GroupingMode::Smart,
                "Manual" => GroupingMode::Manual,
                _ => GroupingMode::Smart,
            })
        })
        .unwrap_or_default();
    let active_task_index = get_state(conn, "active_task_index")
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&i| i < tasks.len());
    LoadedState {
        tasks,
        current_view,
        grouping_mode,
        active_task_index,
    }
}

fn load_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut subtasks_by_uuid = load_subtasks(conn).unwrap_or_default();
    let mut stmt = conn.prepare(
        "SELECT name, notes, project, priority, due_date, due_notified, completed, creation_date, completion_date, uuid
         FROM tasks ORDER BY sort_order ASC",
    )?;
    let tasks = stmt
        .query_map([], |row| {
            let due_str: Option<String> = row.get(4)?;
            let creation_str: String = row.get(7)?;
            let completion_str: Option<String> = row.get(8)?;
            // Legacy rows may have a NULL/empty uuid; mint one so subtasks can
            // key to it. Regenerated ids are persisted on the next save.
            let uuid = row
                .get::<_, Option<String>>(9)?
                .filter(|s| !s.is_empty())
                .unwrap_or_else(new_uuid);
            let subtasks = subtasks_by_uuid.remove(&uuid).unwrap_or_default();
            Ok(Task {
                uuid,
                name: row.get(0)?,
                notes: row.get(1)?,
                project: row.get(2)?,
                priority: priority_from_int(row.get::<_, i64>(3)?),
                due_date: due_str.and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                due_notified: row.get::<_, i64>(5)? != 0,
                completed: row.get::<_, i64>(6)? != 0,
                creation_date: creation_str
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_else(|_| Utc::now()),
                completion_date: completion_str.and_then(|s| s.parse::<DateTime<Utc>>().ok()),
                subtasks,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}

fn load_subtasks(conn: &Connection) -> Result<HashMap<String, Vec<SubTask>>> {
    let mut stmt = conn.prepare(
        "SELECT task_uuid, name, done, creation_date, completion_date
         FROM subtasks ORDER BY sort_order ASC",
    )?;
    let mut map: HashMap<String, Vec<SubTask>> = HashMap::new();
    let rows = stmt.query_map([], |row| {
        let task_uuid: String = row.get(0)?;
        let creation_str: String = row.get(3)?;
        let completion_str: Option<String> = row.get(4)?;
        let sub = SubTask {
            name: row.get(1)?,
            done: row.get::<_, i64>(2)? != 0,
            creation_date: creation_str
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now()),
            completion_date: completion_str.and_then(|s| s.parse::<DateTime<Utc>>().ok()),
        };
        Ok((task_uuid, sub))
    })?;
    for r in rows.flatten() {
        map.entry(r.0).or_default().push(r.1);
    }
    Ok(map)
}

pub fn save_to(conn: &mut Connection, app: &App) -> Result<()> {
    let tx = conn.transaction()?;
    save_tasks(&tx, &app.tasks)?;
    save_subtasks(&tx, &app.tasks)?;
    save_app_state(&tx, app)?;
    tx.commit()
}

fn save_tasks(conn: &Connection, tasks: &[Task]) -> Result<()> {
    conn.execute("DELETE FROM tasks", [])?;
    for (i, task) in tasks.iter().enumerate() {
        conn.execute(
            "INSERT INTO tasks (sort_order, uuid, name, notes, project, priority, due_date, due_notified, completed, creation_date, completion_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                i as i64,
                task.uuid,
                task.name,
                task.notes,
                task.project,
                priority_to_int(task.priority),
                task.due_date.map(|d| d.to_rfc3339()),
                task.due_notified as i64,
                task.completed as i64,
                task.creation_date.to_rfc3339(),
                task.completion_date.map(|d| d.to_rfc3339()),
            ],
        )?;
    }
    Ok(())
}

fn save_subtasks(conn: &Connection, tasks: &[Task]) -> Result<()> {
    conn.execute("DELETE FROM subtasks", [])?;
    for task in tasks {
        for (j, sub) in task.subtasks.iter().enumerate() {
            conn.execute(
                "INSERT INTO subtasks (task_uuid, sort_order, name, done, creation_date, completion_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    task.uuid,
                    j as i64,
                    sub.name,
                    sub.done as i64,
                    sub.creation_date.to_rfc3339(),
                    sub.completion_date.map(|d| d.to_rfc3339()),
                ],
            )?;
        }
    }
    Ok(())
}

fn save_app_state(conn: &Connection, app: &App) -> Result<()> {
    let view_str = match app.current_view {
        View::TaskList => "TaskList",
        View::Statistics => "Statistics",
        View::Settings => "Settings",
        View::TaskDetails => "TaskDetails",
    };
    conn.execute(
        "INSERT OR REPLACE INTO app_state (key, value) VALUES ('current_view', ?1)",
        params![view_str],
    )?;
    let grouping_str = match app.grouping_mode {
        GroupingMode::Smart => "Smart",
        GroupingMode::Project => "Project",
        GroupingMode::Priority => "Priority",
        GroupingMode::Manual => "Manual",
    };
    conn.execute(
        "INSERT OR REPLACE INTO app_state (key, value) VALUES ('grouping_mode', ?1)",
        params![grouping_str],
    )?;
    // Purge the legacy `sort_mode` row so it can't shadow future reads.
    conn.execute("DELETE FROM app_state WHERE key = 'sort_mode'", [])?;
    match app.active_task_index {
        Some(idx) => {
            conn.execute(
                "INSERT OR REPLACE INTO app_state (key, value) VALUES ('active_task_index', ?1)",
                params![idx as i64],
            )?;
        }
        None => {
            conn.execute("DELETE FROM app_state WHERE key = 'active_task_index'", [])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    #[test]
    fn subtasks_round_trip() {
        let dir = std::env::temp_dir().join(format!("todorust_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("rt.db");
        let _ = std::fs::remove_file(&path);

        let mut app = App::default();
        let mut t = Task::new("parent".into(), Some("proj".into()), Priority::High);
        let uuid = t.uuid.clone();
        let mut done_sub = SubTask::new("done one".into());
        done_sub.toggle(); // marks done + completion_date
        t.subtasks.push(done_sub);
        t.subtasks.push(SubTask::new("open one".into()));
        app.tasks.push(t);

        let mut conn = open_and_init(&path).unwrap();
        save_to(&mut conn, &app).unwrap();

        let loaded = load_from(&conn);
        assert_eq!(loaded.tasks.len(), 1);
        let lt = &loaded.tasks[0];
        assert_eq!(lt.uuid, uuid, "uuid must survive save/load");
        assert_eq!(lt.subtasks.len(), 2, "both subtasks persisted in order");
        assert_eq!(lt.subtasks[0].name, "done one");
        assert!(lt.subtasks[0].done && lt.subtasks[0].completion_date.is_some());
        assert_eq!(lt.subtasks[1].name, "open one");
        assert!(!lt.subtasks[1].done);
        assert_eq!(lt.subtask_progress(), Some((1, 2)));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_db_without_uuid_migrates() {
        let dir = std::env::temp_dir().join(format!("todorust_test_leg_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("legacy.db");
        let _ = std::fs::remove_file(&path);

        // Simulate a pre-uuid schema: tasks table without the uuid column.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE tasks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    name TEXT NOT NULL,
                    notes TEXT, project TEXT,
                    priority INTEGER NOT NULL DEFAULT 1,
                    due_date TEXT, due_notified INTEGER NOT NULL DEFAULT 0,
                    completed INTEGER NOT NULL DEFAULT 0,
                    creation_date TEXT NOT NULL, completion_date TEXT
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tasks (sort_order, name, priority, completed, creation_date)
                 VALUES (0, 'old task', 1, 0, ?1)",
                params![Utc::now().to_rfc3339()],
            )
            .unwrap();
        }

        // open_and_init must ALTER in the uuid column without error.
        let conn = open_and_init(&path).unwrap();
        let loaded = load_from(&conn);
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].name, "old task");
        assert!(
            !loaded.tasks[0].uuid.is_empty(),
            "migrated task gets a minted uuid"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn archive_threshold() {
        let mut s = SubTask::new("x".into());
        s.done = true;
        s.completion_date = Some(Utc::now() - chrono::Duration::hours(25));
        assert!(s.is_archived(Utc::now()), ">24h done → archived");
        s.completion_date = Some(Utc::now() - chrono::Duration::hours(1));
        assert!(!s.is_archived(Utc::now()), "recent done → active");
    }
}
