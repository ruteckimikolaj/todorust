use crate::app::ui_state::task_matches_filter;
use crate::settings::Settings;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::fs;
use std::path::PathBuf;

pub mod ui_state;
pub use ui_state::UiState;

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "todorust")
}

pub fn get_data_path() -> Option<PathBuf> {
    project_dirs().map(|d| d.data_local_dir().join("state.json"))
}

pub fn get_db_path() -> Option<PathBuf> {
    project_dirs().map(|d| d.data_local_dir().join("todorust.db"))
}

pub fn get_config_path() -> Option<PathBuf> {
    #[allow(deprecated)]
    std::env::home_dir().map(|h| h.join(".config").join("todorust").join("config.toml"))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Debug, Default)]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
}

impl Priority {
    pub fn title(&self) -> &'static str {
        match self {
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
        }
    }

    /// Single-glyph marker shown next to a task.
    pub fn glyph(&self) -> &'static str {
        match self {
            Priority::Low => "↓",
            Priority::Medium => "•",
            Priority::High => "↑",
        }
    }

    pub fn cycle(&self) -> Priority {
        match self {
            Priority::Low => Priority::Medium,
            Priority::Medium => Priority::High,
            Priority::High => Priority::Low,
        }
    }
}

/// Generate a fresh persistent identifier for a task. Used both by `Task::new`
/// and as a serde default so legacy tasks loaded without a uuid get one.
pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// A subtask has moved to the collapsed "archived" section once it has been
/// `done` for longer than this.
pub const SUBTASK_ARCHIVE_AFTER: chrono::Duration = chrono::Duration::hours(24);

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SubTask {
    pub name: String,
    #[serde(default)]
    pub done: bool,
    pub creation_date: DateTime<Utc>,
    #[serde(default)]
    pub completion_date: Option<DateTime<Utc>>,
}

impl SubTask {
    pub fn new(name: String) -> Self {
        Self {
            name,
            done: false,
            creation_date: Utc::now(),
            completion_date: None,
        }
    }

    /// Flip done state, stamping/clearing the completion date accordingly.
    pub fn toggle(&mut self) {
        self.done = !self.done;
        self.completion_date = if self.done { Some(Utc::now()) } else { None };
    }

    /// True once the subtask has been done for more than [`SUBTASK_ARCHIVE_AFTER`].
    pub fn is_archived(&self, now: DateTime<Utc>) -> bool {
        self.done
            && self
                .completion_date
                .is_some_and(|c| now - c > SUBTASK_ARCHIVE_AFTER)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Task {
    /// Stable identity that survives the wholesale rewrite in `save_tasks`,
    /// so subtasks can be keyed to their parent.
    #[serde(default = "new_uuid")]
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub due_notified: bool,
    pub completed: bool,
    pub creation_date: DateTime<Utc>,
    pub completion_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub subtasks: Vec<SubTask>,
}

impl Task {
    pub fn new(name: String, project: Option<String>, priority: Priority) -> Self {
        Self {
            uuid: new_uuid(),
            name,
            notes: None,
            project,
            priority,
            due_date: None,
            due_notified: false,
            completed: false,
            creation_date: Utc::now(),
            completion_date: None,
            subtasks: Vec::new(),
        }
    }

    /// True when the task has a due date in the past and is not yet done.
    pub fn is_overdue(&self) -> bool {
        !self.completed && self.due_date.is_some_and(|d| d < Utc::now())
    }

    /// `(done, total)` over all subtasks, or `None` when the task has none.
    pub fn subtask_progress(&self) -> Option<(usize, usize)> {
        if self.subtasks.is_empty() {
            return None;
        }
        let done = self.subtasks.iter().filter(|s| s.done).count();
        Some((done, self.subtasks.len()))
    }

    /// Indices into `subtasks` in display order: active first, then archived
    /// (only when `show_archived`). This is the mapping the UI navigates over.
    pub fn visible_subtask_indices(&self, show_archived: bool, now: DateTime<Utc>) -> Vec<usize> {
        let mut active = Vec::new();
        let mut archived = Vec::new();
        for (i, s) in self.subtasks.iter().enumerate() {
            if s.is_archived(now) {
                archived.push(i);
            } else {
                active.push(i);
            }
        }
        if show_archived {
            active.extend(archived);
        }
        active
    }
}

#[derive(Serialize, Deserialize, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum View {
    Dashboard,
    #[default]
    TaskList,
    Statistics,
    Settings,
    TaskDetails,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum SortMode {
    #[default]
    Manual,
    Priority,
    DueDate,
}

impl SortMode {
    pub fn title(&self) -> &'static str {
        match self {
            SortMode::Manual => "Manual",
            SortMode::Priority => "Priority",
            SortMode::DueDate => "Due Date",
        }
    }

    pub fn cycle(&self) -> SortMode {
        match self {
            SortMode::Manual => SortMode::Priority,
            SortMode::Priority => SortMode::DueDate,
            SortMode::DueDate => SortMode::Manual,
        }
    }
}

#[derive(Default)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
    Filtering,
    EditingNotes,
    EditingDue,
    EditingSubtask,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct App {
    #[serde(skip)]
    pub should_quit: bool,
    pub current_view: View,
    pub sort_mode: SortMode,
    pub tasks: Vec<Task>,
    pub active_task_index: Option<usize>,
    #[serde(skip)]
    pub settings: Settings,
}

impl Default for App {
    fn default() -> Self {
        Self {
            should_quit: false,
            current_view: View::TaskList,
            sort_mode: SortMode::Manual,
            tasks: vec![],
            active_task_index: None,
            settings: Settings::default(),
        }
    }
}

impl App {
    pub fn load_with_settings(settings: Settings) -> Self {
        if let Some(db_path) = get_db_path() {
            if let Some(parent) = db_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let is_new_db = !db_path.exists();
            if let Ok(mut conn) = crate::db::open_and_init(&db_path) {
                // One-time migration from legacy JSON on first run
                if is_new_db {
                    if let Some(legacy) = Self::try_load_json() {
                        let _ = crate::db::save_to(&mut conn, &legacy);
                        let mut app = legacy;
                        app.settings = settings;
                        return app;
                    }
                }
                let s = crate::db::load_from(&conn);
                return App {
                    should_quit: false,
                    current_view: s.current_view,
                    sort_mode: s.sort_mode,
                    tasks: s.tasks,
                    active_task_index: s.active_task_index,
                    settings,
                };
            }
        }
        App {
            settings,
            ..App::default()
        }
    }

    fn try_load_json() -> Option<Self> {
        let path = get_data_path()?;
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self) {
        if let Some(db_path) = get_db_path() {
            if let Some(parent) = db_path.parent() {
                if fs::create_dir_all(parent).is_ok() {
                    if let Ok(mut conn) = crate::db::open_and_init(&db_path) {
                        let _ = crate::db::save_to(&mut conn, self);
                    }
                }
            }
        }
        self.settings.save();
    }

    /// Indices of active (incomplete) tasks matching `filter`, in current sort order.
    pub fn ordered_active_indices(&self, filter: &str) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.completed && (filter.is_empty() || task_matches_filter(t, filter)))
            .map(|(i, _)| i)
            .collect();
        match self.sort_mode {
            SortMode::Manual => {}
            SortMode::Priority => {
                indices.sort_by_key(|&i| Reverse(self.tasks[i].priority));
            }
            SortMode::DueDate => {
                // Earliest due first; tasks without a due date sink to the bottom.
                indices.sort_by_key(|&i| {
                    self.tasks[i]
                        .due_date
                        .map(|d| (0, d))
                        .unwrap_or((1, DateTime::<Utc>::MAX_UTC))
                });
            }
        }
        indices
    }

    /// Indices of completed tasks matching `filter`, newest completion first.
    pub fn ordered_completed_indices(&self, filter: &str) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.completed && (filter.is_empty() || task_matches_filter(t, filter)))
            .map(|(i, _)| i)
            .collect();
        indices.sort_by_key(|&i| Reverse(self.tasks[i].completion_date));
        indices
    }

    fn first_active_index(&self) -> Option<usize> {
        self.ordered_active_indices("").first().copied()
    }

    pub fn complete_active_task(&mut self) {
        if let Some(index) = self.active_task_index {
            if let Some(task) = self.tasks.get_mut(index) {
                task.completed = !task.completed;
                if task.completed {
                    task.completion_date = Some(Utc::now());
                    self.active_task_index = self.first_active_index();
                } else {
                    task.completion_date = None;
                }
            }
        }
    }

    pub fn delete_active_task(&mut self) {
        if let Some(index) = self.active_task_index {
            self.tasks.remove(index);
            self.active_task_index = self.first_active_index();
        }
    }

    pub fn cycle_active_priority(&mut self) {
        if let Some(index) = self.active_task_index {
            if let Some(task) = self.tasks.get_mut(index) {
                if !task.completed {
                    task.priority = task.priority.cycle();
                }
            }
        }
    }

    pub fn cycle_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.cycle();
    }

    pub fn move_active_task_up(&mut self) {
        // Reordering only makes sense in manual mode.
        if self.sort_mode != SortMode::Manual {
            return;
        }
        if let Some(index) = self.active_task_index {
            if index > 0 {
                self.tasks.swap(index, index - 1);
                self.active_task_index = Some(index - 1);
            }
        }
    }

    pub fn move_active_task_down(&mut self) {
        if self.sort_mode != SortMode::Manual {
            return;
        }
        if let Some(index) = self.active_task_index {
            if index + 1 < self.tasks.len() {
                self.tasks.swap(index, index + 1);
                self.active_task_index = Some(index + 1);
            }
        }
    }
}
