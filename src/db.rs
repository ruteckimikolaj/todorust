use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use std::path::Path;

use crate::app::{App, Priority, SortMode, Task, View};

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
        CREATE TABLE IF NOT EXISTS app_state (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
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
    pub sort_mode: SortMode,
    pub active_task_index: Option<usize>,
}

pub fn load_from(conn: &Connection) -> LoadedState {
    let tasks = load_tasks(conn).unwrap_or_default();
    let current_view = get_state(conn, "current_view")
        .map(|s| match s.as_str() {
            "Dashboard" => View::Dashboard,
            "Statistics" => View::Statistics,
            _ => View::TaskList,
        })
        .unwrap_or_default();
    let sort_mode = get_state(conn, "sort_mode")
        .map(|s| match s.as_str() {
            "Priority" => SortMode::Priority,
            "DueDate" => SortMode::DueDate,
            _ => SortMode::Manual,
        })
        .unwrap_or_default();
    let active_task_index = get_state(conn, "active_task_index")
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&i| i < tasks.len());
    LoadedState {
        tasks,
        current_view,
        sort_mode,
        active_task_index,
    }
}

fn load_tasks(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(
        "SELECT name, notes, project, priority, due_date, due_notified, completed, creation_date, completion_date
         FROM tasks ORDER BY sort_order ASC",
    )?;
    let tasks = stmt
        .query_map([], |row| {
            let due_str: Option<String> = row.get(4)?;
            let creation_str: String = row.get(7)?;
            let completion_str: Option<String> = row.get(8)?;
            Ok(Task {
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
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}

pub fn save_to(conn: &mut Connection, app: &App) -> Result<()> {
    let tx = conn.transaction()?;
    save_tasks(&tx, &app.tasks)?;
    save_app_state(&tx, app)?;
    tx.commit()
}

fn save_tasks(conn: &Connection, tasks: &[Task]) -> Result<()> {
    conn.execute("DELETE FROM tasks", [])?;
    for (i, task) in tasks.iter().enumerate() {
        conn.execute(
            "INSERT INTO tasks (sort_order, name, notes, project, priority, due_date, due_notified, completed, creation_date, completion_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                i as i64,
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

fn save_app_state(conn: &Connection, app: &App) -> Result<()> {
    let view_str = match app.current_view {
        View::Dashboard => "Dashboard",
        View::TaskList => "TaskList",
        View::Statistics => "Statistics",
        View::Settings => "Settings",
        View::TaskDetails => "TaskDetails",
    };
    conn.execute(
        "INSERT OR REPLACE INTO app_state (key, value) VALUES ('current_view', ?1)",
        params![view_str],
    )?;
    let sort_str = match app.sort_mode {
        SortMode::Manual => "Manual",
        SortMode::Priority => "Priority",
        SortMode::DueDate => "DueDate",
    };
    conn.execute(
        "INSERT OR REPLACE INTO app_state (key, value) VALUES ('sort_mode', ?1)",
        params![sort_str],
    )?;
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
