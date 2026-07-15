use chrono::{Local, NaiveDateTime, TimeZone, Utc};
use ratatui_textarea::TextArea;

use super::{App, InputMode, Priority, SubTask, Task, View};
use crate::settings::ColorTheme;

/// Splits `"Buy milk @work"` → `("Buy milk", Some("work"))`.
/// The `@tag` can appear anywhere; it is stripped from the name.
pub fn parse_project(input: &str) -> (String, Option<String>) {
    if let Some(at) = input.rfind('@') {
        let rest = &input[at + 1..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .unwrap_or(rest.len());
        if end > 0 {
            let project = rest[..end].to_string();
            let name = format!("{}{}", &input[..at], &rest[end..])
                .trim()
                .to_string();
            if !name.is_empty() {
                return (name, Some(project));
            }
        }
    }
    (input.trim().to_string(), None)
}

pub fn task_matches_filter(task: &Task, filter: &str) -> bool {
    task.name.to_lowercase().contains(filter)
        || task
            .notes
            .as_deref()
            .is_some_and(|n| n.to_lowercase().contains(filter))
        || task.project.as_deref().is_some_and(|p| {
            let tag = format!("@{}", p.to_lowercase());
            tag.contains(filter) || p.to_lowercase().contains(filter)
        })
}

/// Format for the due-date editor: `2026-07-20 14:30`.
pub const DUE_FORMAT: &str = "%Y-%m-%d %H:%M";

/// Parse a due-date string entered by the user into UTC. Empty input clears the date.
pub fn parse_due(input: &str) -> Result<Option<chrono::DateTime<Utc>>, ()> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match NaiveDateTime::parse_from_str(trimmed, DUE_FORMAT) {
        Ok(naive) => match Local.from_local_datetime(&naive).single() {
            Some(local) => Ok(Some(local.with_timezone(&Utc))),
            None => Err(()),
        },
        Err(_) => Err(()),
    }
}

const SETTINGS_ROW_COUNT: usize = 3;

/// The focusable fields of the [`EditSheet`], in Tab order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SheetField {
    Name,
    Project,
    Priority,
    Due,
    Notes,
}

impl SheetField {
    const ORDER: [SheetField; 5] = [
        SheetField::Name,
        SheetField::Project,
        SheetField::Priority,
        SheetField::Due,
        SheetField::Notes,
    ];

    pub fn next(self) -> Self {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// State for the all-attributes edit sheet (Phase 2). One modal replaces the
/// former standalone rename / priority / due / notes keybindings.
pub struct EditSheet {
    pub task_index: usize,
    pub field: SheetField,
    pub name: String,
    pub project: String,
    pub priority: Priority,
    pub due: String,
    pub due_error: bool,
    pub notes: TextArea<'static>,
}

pub struct UiState {
    pub settings_selection: usize,
    pub completed_task_list_state: Option<usize>,
    pub previous_view: View,
    pub input_mode: InputMode,
    pub current_input: String,
    pub filter_input: String,
    pub editing_task_index: Option<usize>,
    pub notes_textarea: Option<TextArea<'static>>,
    pub editing_notes_task_index: Option<usize>,
    /// When `Some`, a subtask row under the active parent is highlighted
    /// (index into that parent's visible-subtask list).
    pub selected_subtask: Option<usize>,
    pub subtask_input: String,
    pub editing_subtask_parent: Option<usize>,
    /// Reveal the collapsed archived section of the active parent's checklist.
    pub show_archived: bool,
    /// When true, the `?` help overlay is drawn over the task list.
    pub show_help: bool,
    /// When true, a delete-confirmation prompt is drawn; `y` confirms.
    pub confirm_delete: bool,
    /// The open edit sheet, if any (`InputMode::EditingSheet`).
    pub edit_sheet: Option<EditSheet>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            settings_selection: 0,
            completed_task_list_state: None,
            previous_view: View::TaskList,
            input_mode: InputMode::Normal,
            current_input: String::new(),
            filter_input: String::new(),
            editing_task_index: None,
            notes_textarea: None,
            editing_notes_task_index: None,
            selected_subtask: None,
            subtask_input: String::new(),
            editing_subtask_parent: None,
            show_archived: false,
            show_help: false,
            confirm_delete: false,
            edit_sheet: None,
        }
    }
}

impl UiState {
    pub fn next_setting(&mut self) {
        self.settings_selection = (self.settings_selection + 1) % SETTINGS_ROW_COUNT;
    }

    pub fn previous_setting(&mut self) {
        if self.settings_selection > 0 {
            self.settings_selection -= 1;
        } else {
            self.settings_selection = SETTINGS_ROW_COUNT - 1;
        }
    }

    pub fn modify_setting(&mut self, app: &mut App, increase: bool) {
        let delta: i64 = if increase { 1 } else { -1 };
        match self.settings_selection {
            0 => {
                let mut themes = vec![
                    ColorTheme::Default,
                    ColorTheme::Dracula,
                    ColorTheme::Solarized,
                    ColorTheme::Nord,
                    ColorTheme::GruvboxDark,
                    ColorTheme::Cyberpunk,
                ];
                if app.settings.custom_theme.is_some() {
                    themes.push(ColorTheme::Custom);
                }
                let cur = themes
                    .iter()
                    .position(|t| *t == app.settings.theme)
                    .unwrap_or(0);
                let len = themes.len() as i64;
                let next = ((cur as i64 + delta).rem_euclid(len)) as usize;
                app.settings.theme = themes[next];
            }
            1 => app.settings.desktop_notifications = !app.settings.desktop_notifications,
            2 => {
                app.settings.default_priority = if increase {
                    app.settings.default_priority.cycle()
                } else {
                    // cycle backwards
                    app.settings.default_priority.cycle().cycle()
                };
            }
            _ => {}
        }
    }

    // --- Completed-task navigation (Statistics view) ---

    fn filtered_completed_count(&self, app: &App) -> usize {
        app.ordered_completed_indices(&self.filter_input.to_lowercase())
            .len()
    }

    pub fn next_completed_task(&mut self, app: &App) {
        let count = self.filtered_completed_count(app);
        if count == 0 {
            return;
        }
        let i = self
            .completed_task_list_state
            .map_or(0, |i| (i + 1) % count);
        self.completed_task_list_state = Some(i);
    }

    pub fn previous_completed_task(&mut self, app: &App) {
        let count = self.filtered_completed_count(app);
        if count == 0 {
            return;
        }
        let i = self
            .completed_task_list_state
            .map_or(0, |i| if i == 0 { count - 1 } else { i - 1 });
        self.completed_task_list_state = Some(i);
    }

    pub fn delete_selected_completed_task(&mut self, app: &mut App) {
        if let Some(selected) = self.completed_task_list_state {
            let completed_indices =
                app.ordered_completed_indices(&self.filter_input.to_lowercase());
            if let Some(&idx) = completed_indices.get(selected) {
                app.tasks.remove(idx);
                if let Some(active) = app.active_task_index {
                    if active > idx {
                        app.active_task_index = Some(active - 1);
                    }
                }
                self.completed_task_list_state = None;
            }
        }
    }

    // --- Notes editor ---

    fn open_notes_for_task(&mut self, idx: usize, app: &App) {
        if let Some(task) = app.tasks.get(idx) {
            let lines: Vec<String> = task
                .notes
                .as_deref()
                .unwrap_or("")
                .lines()
                .map(|l| l.to_owned())
                .collect();
            let mut textarea = if lines.is_empty() {
                TextArea::default()
            } else {
                TextArea::new(lines)
            };
            textarea.set_placeholder_text("Type your notes here…");
            self.notes_textarea = Some(textarea);
            self.editing_notes_task_index = Some(idx);
            self.input_mode = InputMode::EditingNotes;
        }
    }

    // Open notes editor for the selected completed task (called from TaskDetails)
    pub fn start_edit_notes(&mut self, app: &App) {
        if let Some(selected) = self.completed_task_list_state {
            let completed_indices =
                app.ordered_completed_indices(&self.filter_input.to_lowercase());
            if let Some(&idx) = completed_indices.get(selected) {
                self.open_notes_for_task(idx, app);
            }
        }
    }

    pub fn submit_notes(&mut self, app: &mut App) {
        if let (Some(textarea), Some(idx)) = (
            self.notes_textarea.take(),
            self.editing_notes_task_index.take(),
        ) {
            if let Some(task) = app.tasks.get_mut(idx) {
                let text = textarea.lines().join("\n");
                task.notes = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
        }
        self.input_mode = InputMode::Normal;
    }

    pub fn cancel_notes(&mut self) {
        self.notes_textarea = None;
        self.editing_notes_task_index = None;
        self.input_mode = InputMode::Normal;
    }

    // --- Active-task navigation (sort- and filter-aware) ---

    /// Move selection down. Steps into the active parent's subtasks, then on
    /// to the next parent once past the last subtask.
    pub fn next_active_task(&mut self, app: &mut App) {
        let indices = app.active_display_order(&self.filter_input.to_lowercase());
        if indices.is_empty() {
            app.active_task_index = None;
            self.selected_subtask = None;
            return;
        }
        let now = Utc::now();
        let cur = app.active_task_index.unwrap_or(usize::MAX);
        match indices.iter().position(|&i| i == cur) {
            None => {
                app.active_task_index = Some(indices[0]);
                self.selected_subtask = None;
            }
            Some(p) => {
                let vis = app.tasks[indices[p]]
                    .visible_subtask_indices(self.show_archived, now)
                    .len();
                match self.selected_subtask {
                    Some(s) if s + 1 < vis => self.selected_subtask = Some(s + 1),
                    Some(_) => {
                        app.active_task_index = Some(indices[(p + 1) % indices.len()]);
                        self.selected_subtask = None;
                    }
                    None if vis > 0 => self.selected_subtask = Some(0),
                    None => {
                        app.active_task_index = Some(indices[(p + 1) % indices.len()]);
                        self.selected_subtask = None;
                    }
                }
            }
        }
    }

    /// Move selection up. Steps back out of subtasks to the parent row, then on
    /// to the previous parent.
    pub fn previous_active_task(&mut self, app: &mut App) {
        let indices = app.active_display_order(&self.filter_input.to_lowercase());
        if indices.is_empty() {
            app.active_task_index = None;
            self.selected_subtask = None;
            return;
        }
        let cur = app.active_task_index.unwrap_or(usize::MAX);
        match indices.iter().position(|&i| i == cur) {
            None => {
                app.active_task_index = Some(indices[0]);
                self.selected_subtask = None;
            }
            Some(p) => match self.selected_subtask {
                Some(0) => self.selected_subtask = None,
                Some(s) => self.selected_subtask = Some(s - 1),
                None => {
                    let prev = if p == 0 { indices.len() - 1 } else { p - 1 };
                    app.active_task_index = Some(indices[prev]);
                    self.selected_subtask = None;
                }
            },
        }
    }

    // --- Subtasks ---

    /// Open the inline input to add a subtask under the active parent.
    pub fn start_add_subtask(&mut self, app: &App) {
        if let Some(idx) = app.active_task_index {
            if app.tasks.get(idx).is_some_and(|t| !t.completed) {
                self.editing_subtask_parent = Some(idx);
                self.subtask_input.clear();
                self.input_mode = InputMode::EditingSubtask;
            }
        }
    }

    pub fn submit_subtask(&mut self, app: &mut App) {
        if let Some(idx) = self.editing_subtask_parent.take() {
            let name = self.subtask_input.trim().to_string();
            if !name.is_empty() {
                if let Some(task) = app.tasks.get_mut(idx) {
                    task.subtasks.push(SubTask::new(name));
                }
            }
        }
        self.subtask_input.clear();
        self.input_mode = InputMode::Normal;
    }

    pub fn cancel_subtask(&mut self) {
        self.subtask_input.clear();
        self.editing_subtask_parent = None;
        self.input_mode = InputMode::Normal;
    }

    /// Toggle the `done` state of the currently highlighted subtask.
    pub fn toggle_selected_subtask(&mut self, app: &mut App) {
        if let (Some(pidx), Some(sel)) = (app.active_task_index, self.selected_subtask) {
            let now = Utc::now();
            if let Some(task) = app.tasks.get_mut(pidx) {
                let vis = task.visible_subtask_indices(self.show_archived, now);
                if let Some(&si) = vis.get(sel) {
                    if let Some(sub) = task.subtasks.get_mut(si) {
                        sub.toggle();
                    }
                }
            }
        }
    }

    // --- New / rename task ---

    // --- Edit sheet (all attributes in one modal) ---

    /// Open the edit sheet for the active task, seeded from its current values.
    pub fn open_edit_sheet(&mut self, app: &App) {
        if let Some(idx) = app.active_task_index {
            if let Some(task) = app.tasks.get(idx) {
                if !task.completed {
                    let lines: Vec<String> = task
                        .notes
                        .as_deref()
                        .unwrap_or("")
                        .lines()
                        .map(|l| l.to_owned())
                        .collect();
                    let mut notes = if lines.is_empty() {
                        TextArea::default()
                    } else {
                        TextArea::new(lines)
                    };
                    notes.set_placeholder_text("Notes…");
                    self.edit_sheet = Some(EditSheet {
                        task_index: idx,
                        field: SheetField::Name,
                        name: task.name.clone(),
                        project: task.project.clone().unwrap_or_default(),
                        priority: task.priority,
                        due: task
                            .due_date
                            .map(|d| d.with_timezone(&Local).format(DUE_FORMAT).to_string())
                            .unwrap_or_default(),
                        due_error: false,
                        notes,
                    });
                    self.input_mode = InputMode::EditingSheet;
                }
            }
        }
    }

    /// Mutable handle to the buffer of the currently focused text field, if the
    /// focused field is a plain text field (Name/Project/Due).
    pub fn sheet_text_field_mut(&mut self) -> Option<&mut String> {
        let sheet = self.edit_sheet.as_mut()?;
        match sheet.field {
            SheetField::Name => Some(&mut sheet.name),
            SheetField::Project => Some(&mut sheet.project),
            SheetField::Due => {
                sheet.due_error = false;
                Some(&mut sheet.due)
            }
            _ => None,
        }
    }

    /// Validate and write the sheet back to its task. Keeps the sheet open on a
    /// bad due date or empty name so the user can fix it.
    pub fn submit_sheet(&mut self, app: &mut App) {
        let Some(sheet) = self.edit_sheet.as_mut() else {
            self.input_mode = InputMode::Normal;
            return;
        };
        let name = sheet.name.trim().to_string();
        if name.is_empty() {
            sheet.field = SheetField::Name;
            return;
        }
        let due = match parse_due(&sheet.due) {
            Ok(d) => d,
            Err(()) => {
                sheet.due_error = true;
                sheet.field = SheetField::Due;
                return;
            }
        };
        let idx = sheet.task_index;
        let project = {
            let p = sheet.project.trim();
            if p.is_empty() {
                None
            } else {
                Some(p.trim_start_matches('@').to_string())
            }
        };
        let priority = sheet.priority;
        let notes_text = sheet.notes.lines().join("\n");
        if let Some(task) = app.tasks.get_mut(idx) {
            task.name = name;
            task.project = project;
            task.priority = priority;
            if task.due_date != due {
                task.due_date = due;
                task.due_notified = false;
            }
            task.notes = if notes_text.trim().is_empty() {
                None
            } else {
                Some(notes_text)
            };
        }
        self.edit_sheet = None;
        self.input_mode = InputMode::Normal;
    }

    pub fn cancel_sheet(&mut self) {
        self.edit_sheet = None;
        self.input_mode = InputMode::Normal;
    }

    pub fn submit_task(&mut self, app: &mut App) {
        if let Some(idx) = self.editing_task_index.take() {
            if !self.current_input.is_empty() {
                let (name, project) = parse_project(&self.current_input);
                if let Some(task) = app.tasks.get_mut(idx) {
                    task.name = name;
                    task.project = project;
                }
            }
            self.current_input.clear();
            self.input_mode = InputMode::Normal;
        } else {
            if !self.current_input.is_empty() {
                let (name, project) = parse_project(&self.current_input);
                let priority = app.settings.default_priority;
                app.tasks.push(Task::new(name, project, priority));
                self.current_input.clear();
                if app.active_task_index.is_none() {
                    app.active_task_index = Some(app.tasks.len() - 1);
                }
            }
            self.input_mode = InputMode::Normal;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Priority, SubTask, Task};

    fn app_with_two() -> App {
        let mut app = App::default();
        let mut p0 = Task::new("p0".into(), None, Priority::Medium);
        p0.subtasks.push(SubTask::new("s0".into()));
        p0.subtasks.push(SubTask::new("s1".into()));
        app.tasks.push(p0);
        app.tasks
            .push(Task::new("p1".into(), None, Priority::Medium));
        app.active_task_index = Some(0);
        app
    }

    #[test]
    fn down_steps_into_subtasks_then_next_parent() {
        let mut app = app_with_two();
        let mut ui = UiState::default();
        // parent 0 selected, no subtask
        assert_eq!(
            (app.active_task_index, ui.selected_subtask),
            (Some(0), None)
        );
        ui.next_active_task(&mut app); // -> s0
        assert_eq!(
            (app.active_task_index, ui.selected_subtask),
            (Some(0), Some(0))
        );
        ui.next_active_task(&mut app); // -> s1
        assert_eq!(
            (app.active_task_index, ui.selected_subtask),
            (Some(0), Some(1))
        );
        ui.next_active_task(&mut app); // past last -> parent 1
        assert_eq!(
            (app.active_task_index, ui.selected_subtask),
            (Some(1), None)
        );
    }

    #[test]
    fn up_steps_back_out_of_subtasks() {
        let mut app = app_with_two();
        let mut ui = UiState::default();
        ui.selected_subtask = Some(1); // on s1 of parent 0
        ui.previous_active_task(&mut app); // -> s0
        assert_eq!(
            (app.active_task_index, ui.selected_subtask),
            (Some(0), Some(0))
        );
        ui.previous_active_task(&mut app); // -> parent row
        assert_eq!(
            (app.active_task_index, ui.selected_subtask),
            (Some(0), None)
        );
        ui.previous_active_task(&mut app); // wrap to parent 1
        assert_eq!(
            (app.active_task_index, ui.selected_subtask),
            (Some(1), None)
        );
    }

    #[test]
    fn edit_sheet_writes_all_fields_back() {
        let mut app = App::default();
        app.tasks.push(Task::new("old".into(), None, Priority::Low));
        app.active_task_index = Some(0);
        let mut ui = UiState::default();
        ui.open_edit_sheet(&app);
        let sheet = ui.edit_sheet.as_mut().unwrap();
        sheet.name = "new name".into();
        sheet.project = "work".into();
        sheet.priority = Priority::High;
        sheet.due = "2026-08-01 09:30".into();
        ui.submit_sheet(&mut app);
        assert!(ui.edit_sheet.is_none());
        let t = &app.tasks[0];
        assert_eq!(t.name, "new name");
        assert_eq!(t.project.as_deref(), Some("work"));
        assert_eq!(t.priority, Priority::High);
        assert!(t.due_date.is_some());
    }

    #[test]
    fn edit_sheet_bad_due_keeps_sheet_open() {
        let mut app = App::default();
        app.tasks
            .push(Task::new("t".into(), None, Priority::Medium));
        app.active_task_index = Some(0);
        let mut ui = UiState::default();
        ui.open_edit_sheet(&app);
        ui.edit_sheet.as_mut().unwrap().due = "not a date".into();
        ui.submit_sheet(&mut app);
        // Sheet stays open, flagged, focused on Due; task unchanged.
        let sheet = ui.edit_sheet.as_ref().expect("sheet still open");
        assert!(sheet.due_error);
        assert_eq!(sheet.field, SheetField::Due);
        assert!(app.tasks[0].due_date.is_none());
    }

    #[test]
    fn toggle_selected_subtask_marks_done() {
        let mut app = app_with_two();
        let mut ui = UiState::default();
        ui.selected_subtask = Some(0);
        ui.toggle_selected_subtask(&mut app);
        assert!(app.tasks[0].subtasks[0].done);
        assert!(app.tasks[0].subtasks[0].completion_date.is_some());
        ui.toggle_selected_subtask(&mut app);
        assert!(!app.tasks[0].subtasks[0].done);
        assert!(app.tasks[0].subtasks[0].completion_date.is_none());
    }
}
