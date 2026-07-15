use crate::app::ui_state::task_matches_filter;
use crate::settings::Settings;
use chrono::{DateTime, Local, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BTreeMap;
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
    #[default]
    TaskList,
    Statistics,
    Settings,
    TaskDetails,
}

/// How the Task List groups its rows (Phase 3). Replaces the flat `SortMode`
/// with a section-based agenda; `Manual` keeps the legacy user-ordered flat list.
#[derive(Serialize, Deserialize, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum GroupingMode {
    #[default]
    Smart,
    Project,
    Priority,
    Manual,
}

impl GroupingMode {
    pub fn title(&self) -> &'static str {
        match self {
            GroupingMode::Smart => "Smart",
            GroupingMode::Project => "Project",
            GroupingMode::Priority => "Priority",
            GroupingMode::Manual => "Manual",
        }
    }

    pub fn cycle(&self) -> GroupingMode {
        match self {
            GroupingMode::Smart => GroupingMode::Project,
            GroupingMode::Project => GroupingMode::Priority,
            GroupingMode::Priority => GroupingMode::Manual,
            GroupingMode::Manual => GroupingMode::Smart,
        }
    }
}

/// Visual/semantic hint for how to colour a section header.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SectionTone {
    Overdue,
    Today,
    Upcoming,
    NoDate,
    High,
    Medium,
    Low,
    Neutral,
}

/// A group of tasks under a single header, in display order (task indices into
/// [`App::tasks`]). Used by the Task List for both rendering and navigation.
#[derive(Clone, Debug)]
pub struct Section {
    pub label: String,
    pub tone: SectionTone,
    pub indices: Vec<usize>,
}

#[derive(Default)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
    Filtering,
    EditingNotes,
    EditingSubtask,
    /// The all-attributes edit sheet is open (see [`ui_state::EditSheet`]).
    EditingSheet,
    /// A one-line prompt asking for a date shortcut to reschedule the active
    /// task (see [`ui_state::UiState::reschedule_input`]).
    Rescheduling,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct App {
    #[serde(skip)]
    pub should_quit: bool,
    pub current_view: View,
    pub grouping_mode: GroupingMode,
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
            grouping_mode: GroupingMode::Smart,
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
                    grouping_mode: s.grouping_mode,
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

    /// Group active (incomplete) tasks matching `filter` into sections for
    /// display, in section-then-task order. This is the source of truth for
    /// both the Task List rendering and its navigation.
    pub fn grouped_active_sections(&self, filter: &str) -> Vec<Section> {
        let mut indices: Vec<usize> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.completed && (filter.is_empty() || task_matches_filter(t, filter)))
            .map(|(i, _)| i)
            .collect();

        match self.grouping_mode {
            GroupingMode::Manual => {
                // One flat, unlabeled section preserving raw order.
                vec![Section {
                    label: String::new(),
                    tone: SectionTone::Neutral,
                    indices,
                }]
            }
            GroupingMode::Priority => {
                indices.sort_by_key(|&i| Reverse(self.tasks[i].priority));
                let mut high = Vec::new();
                let mut med = Vec::new();
                let mut low = Vec::new();
                for i in indices {
                    match self.tasks[i].priority {
                        Priority::High => high.push(i),
                        Priority::Medium => med.push(i),
                        Priority::Low => low.push(i),
                    }
                }
                let mut out = Vec::new();
                if !high.is_empty() {
                    out.push(Section {
                        label: "↑ High".into(),
                        tone: SectionTone::High,
                        indices: high,
                    });
                }
                if !med.is_empty() {
                    out.push(Section {
                        label: "• Medium".into(),
                        tone: SectionTone::Medium,
                        indices: med,
                    });
                }
                if !low.is_empty() {
                    out.push(Section {
                        label: "↓ Low".into(),
                        tone: SectionTone::Low,
                        indices: low,
                    });
                }
                out
            }
            GroupingMode::Project => {
                let mut by_project: BTreeMap<String, Vec<usize>> = BTreeMap::new();
                let mut no_project: Vec<usize> = Vec::new();
                for i in indices {
                    match &self.tasks[i].project {
                        Some(p) => by_project.entry(p.clone()).or_default().push(i),
                        None => no_project.push(i),
                    }
                }
                let mut out: Vec<Section> = by_project
                    .into_iter()
                    .map(|(name, indices)| Section {
                        label: format!("@{}", name),
                        tone: SectionTone::Neutral,
                        indices,
                    })
                    .collect();
                if !no_project.is_empty() {
                    out.push(Section {
                        label: "No project".into(),
                        tone: SectionTone::NoDate,
                        indices: no_project,
                    });
                }
                out
            }
            GroupingMode::Smart => {
                let today = Local::now().date_naive();
                let mut overdue = Vec::new();
                let mut today_tasks = Vec::new();
                let mut upcoming = Vec::new();
                let mut no_date = Vec::new();
                for i in indices {
                    let task = &self.tasks[i];
                    match task.due_date {
                        None => no_date.push(i),
                        Some(due) => {
                            let local_date = due.with_timezone(&Local).date_naive();
                            if task.is_overdue() {
                                overdue.push(i);
                            } else if local_date == today {
                                today_tasks.push(i);
                            } else {
                                upcoming.push(i);
                            }
                        }
                    }
                }
                // Within-section ordering: earliest due first for dated
                // sections; keep priority order for undated.
                let due_key = |i: usize| self.tasks[i].due_date.unwrap_or(DateTime::<Utc>::MAX_UTC);
                overdue.sort_by_key(|&i| due_key(i));
                today_tasks.sort_by_key(|&i| due_key(i));
                upcoming.sort_by_key(|&i| due_key(i));
                no_date.sort_by_key(|&i| Reverse(self.tasks[i].priority));

                let mut out = Vec::new();
                if !overdue.is_empty() {
                    out.push(Section {
                        label: "⚠ Overdue".into(),
                        tone: SectionTone::Overdue,
                        indices: overdue,
                    });
                }
                if !today_tasks.is_empty() {
                    out.push(Section {
                        label: "● Today".into(),
                        tone: SectionTone::Today,
                        indices: today_tasks,
                    });
                }
                if !upcoming.is_empty() {
                    out.push(Section {
                        label: "↗ Upcoming".into(),
                        tone: SectionTone::Upcoming,
                        indices: upcoming,
                    });
                }
                if !no_date.is_empty() {
                    out.push(Section {
                        label: "◦ No date".into(),
                        tone: SectionTone::NoDate,
                        indices: no_date,
                    });
                }
                out
            }
        }
    }

    /// Flat display order of active tasks (concatenation of every section's
    /// indices). Navigation walks this list.
    pub fn active_display_order(&self, filter: &str) -> Vec<usize> {
        self.grouped_active_sections(filter)
            .into_iter()
            .flat_map(|s| s.indices)
            .collect()
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
        self.active_display_order("").first().copied()
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

    /// Set the active task's priority directly (used by the `1`/`2`/`3` shortcuts).
    pub fn set_active_priority(&mut self, priority: Priority) {
        if let Some(index) = self.active_task_index {
            if let Some(task) = self.tasks.get_mut(index) {
                if !task.completed {
                    task.priority = priority;
                }
            }
        }
    }

    /// Overwrite the active task's due date (used by `t`/`T`/`w`/`r` presets).
    /// `None` clears the due date. The `due_notified` flag is reset so the
    /// notification fires afresh once the new date passes.
    pub fn set_active_due(&mut self, due: Option<DateTime<Utc>>) {
        if let Some(index) = self.active_task_index {
            if let Some(task) = self.tasks.get_mut(index) {
                if !task.completed {
                    task.due_date = due;
                    task.due_notified = false;
                }
            }
        }
    }

    pub fn cycle_grouping_mode(&mut self) {
        self.grouping_mode = self.grouping_mode.cycle();
    }

    pub fn move_active_task_up(&mut self) {
        // Reordering only makes sense in manual mode.
        if self.grouping_mode != GroupingMode::Manual {
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
        if self.grouping_mode != GroupingMode::Manual {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn t(name: &str, priority: Priority) -> Task {
        Task::new(name.into(), None, priority)
    }

    fn with_project(mut task: Task, project: &str) -> Task {
        task.project = Some(project.into());
        task
    }

    fn with_due(mut task: Task, offset: chrono::Duration) -> Task {
        task.due_date = Some(Utc::now() + offset);
        task
    }

    fn app_with(mode: GroupingMode) -> App {
        App {
            grouping_mode: mode,
            ..App::default()
        }
    }

    #[test]
    fn smart_grouping_splits_overdue_today_upcoming_no_date() {
        let mut app = app_with(GroupingMode::Smart);
        app.tasks.push(with_due(
            t("late", Priority::Medium),
            chrono::Duration::hours(-3),
        ));
        // Something later today, well after `now`, so it lands in Today.
        app.tasks.push(with_due(
            t("later today", Priority::Medium),
            chrono::Duration::minutes(30),
        ));
        app.tasks.push(with_due(
            t("next week", Priority::Medium),
            chrono::Duration::days(7),
        ));
        app.tasks.push(t("someday", Priority::Medium));

        let sections = app.grouped_active_sections("");
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["⚠ Overdue", "● Today", "↗ Upcoming", "◦ No date"]
        );
        for s in &sections {
            assert_eq!(s.indices.len(), 1, "one task per section, got {:?}", s);
        }
    }

    #[test]
    fn project_grouping_buckets_tasks_and_appends_no_project() {
        let mut app = app_with(GroupingMode::Project);
        app.tasks
            .push(with_project(t("a1", Priority::Medium), "alpha"));
        app.tasks.push(t("floating", Priority::Medium));
        app.tasks
            .push(with_project(t("b1", Priority::Medium), "beta"));
        app.tasks
            .push(with_project(t("a2", Priority::Medium), "alpha"));

        let sections = app.grouped_active_sections("");
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        // BTreeMap sorts projects alphabetically; "No project" always trails.
        assert_eq!(labels, vec!["@alpha", "@beta", "No project"]);
        assert_eq!(sections[0].indices.len(), 2, "@alpha has two tasks");
        assert_eq!(sections[2].indices.len(), 1);
    }

    #[test]
    fn priority_grouping_orders_high_first() {
        let mut app = app_with(GroupingMode::Priority);
        app.tasks.push(t("low", Priority::Low));
        app.tasks.push(t("high", Priority::High));
        app.tasks.push(t("med", Priority::Medium));

        let sections = app.grouped_active_sections("");
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, vec!["↑ High", "• Medium", "↓ Low"]);
    }

    #[test]
    fn manual_grouping_is_one_flat_unlabeled_section() {
        let mut app = app_with(GroupingMode::Manual);
        app.tasks.push(t("a", Priority::Low));
        app.tasks.push(t("b", Priority::High));

        let sections = app.grouped_active_sections("");
        assert_eq!(sections.len(), 1);
        assert!(sections[0].label.is_empty(), "Manual has no header");
        assert_eq!(
            sections[0].indices,
            vec![0, 1],
            "raw insertion order preserved"
        );
    }

    #[test]
    fn display_order_concatenates_sections() {
        let mut app = app_with(GroupingMode::Priority);
        app.tasks.push(t("low", Priority::Low)); // idx 0
        app.tasks.push(t("high", Priority::High)); // idx 1
        app.tasks.push(t("med", Priority::Medium)); // idx 2
                                                    // High → Medium → Low: 1, 2, 0.
        assert_eq!(app.active_display_order(""), vec![1, 2, 0]);
    }

    #[test]
    fn grouped_sections_apply_filter() {
        let mut app = app_with(GroupingMode::Smart);
        app.tasks.push(t("write report", Priority::Medium));
        app.tasks.push(t("buy milk", Priority::Medium));
        let sections = app.grouped_active_sections("milk");
        let all: Vec<usize> = sections.iter().flat_map(|s| s.indices.clone()).collect();
        assert_eq!(all, vec![1]);
    }

    #[test]
    fn manual_reorder_only_active_when_grouping_is_manual() {
        let mut app = app_with(GroupingMode::Smart);
        app.tasks.push(t("a", Priority::Medium));
        app.tasks.push(t("b", Priority::Medium));
        app.active_task_index = Some(0);
        // Smart grouping: reorder is a no-op.
        app.move_active_task_down();
        assert_eq!(app.active_task_index, Some(0));
        // Manual grouping: the swap goes through.
        app.grouping_mode = GroupingMode::Manual;
        app.move_active_task_down();
        assert_eq!(app.active_task_index, Some(1));
        assert_eq!(app.tasks[0].name, "b");
        assert_eq!(app.tasks[1].name, "a");
    }
}
