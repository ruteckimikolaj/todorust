use chrono::{
    DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
    Weekday,
};
use ratatui_textarea::TextArea;

use super::{App, InputMode, Priority, Recurrence, SubTask, Task, View};
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

/// Result of one-line natural-language capture parsing. All fields are
/// optional except `name`; unrecognised tokens stay in the name so the user
/// isn't surprised by silently-dropped text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuickAdd {
    pub name: String,
    pub project: Option<String>,
    pub priority: Option<Priority>,
    pub due: Option<DateTime<Utc>>,
    pub recurrence: Option<Recurrence>,
}

fn parse_priority_token(body: &str) -> Option<Priority> {
    match body.to_ascii_lowercase().as_str() {
        "1" | "l" | "low" => Some(Priority::Low),
        "2" | "m" | "med" | "medium" => Some(Priority::Medium),
        "3" | "h" | "high" => Some(Priority::High),
        _ => None,
    }
}

/// Map a shortcut string to a [`Recurrence`]. Recognised forms (case-insensitive):
/// `daily`, `weekly`, `monthly`; weekday names (`mon`..`sun`, long or short);
/// and `Nd` / `Nw` / `Nm` for every-N-days / -weeks / -months.
///
/// The edit-sheet parser and the `%` token in quick-add both feed through this.
pub fn parse_recurrence(input: &str) -> Option<Recurrence> {
    let s = input.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    match s.as_str() {
        "daily" | "day" => return Some(Recurrence::EveryDays(1)),
        "weekly" | "week" => return Some(Recurrence::EveryWeeks(1)),
        "monthly" | "month" => return Some(Recurrence::EveryMonths(1)),
        _ => {}
    }
    if let Some(wd) = weekday_from_str(&s) {
        return Some(Recurrence::Weekly(wd));
    }
    // `Nd` / `Nw` / `Nm` — the trailing char picks the unit.
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let (num_part, unit) = s.split_at(s.len() - 1);
        if let Ok(n) = num_part.parse::<u32>() {
            if n >= 1 {
                return match unit {
                    "d" => Some(Recurrence::EveryDays(n)),
                    "w" => Some(Recurrence::EveryWeeks(n)),
                    "m" => Some(Recurrence::EveryMonths(n)),
                    _ => None,
                };
            }
        }
    }
    None
}

/// Map a shortcut string (case-insensitive) to a concrete due date. Recognised
/// forms: `today` / `tod`, `tomorrow` / `tmrw`, weekday names (`mon`..`sun`,
/// long or short), `next-week` / `next_week` / `nextweek` / `nw`, and any
/// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM` literal.
///
/// Bare dates use a sensible default time: 23:59 local for *today* (so the
/// task doesn't immediately go overdue), 09:00 local for future dates.
pub fn parse_date_shortcut(input: &str) -> Option<DateTime<Utc>> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    // Explicit datetime always wins.
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, DUE_FORMAT) {
        return Local
            .from_local_datetime(&naive)
            .single()
            .map(|d| d.with_timezone(&Utc));
    }
    // Bare YYYY-MM-DD → 09:00 that morning.
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return with_default_time(date);
    }

    let today = Local::now().date_naive();
    let target: Option<NaiveDate> = match s.to_ascii_lowercase().as_str() {
        "today" | "tod" | "t" => Some(today),
        "tomorrow" | "tmrw" | "tom" => Some(today + Duration::days(1)),
        "next-week" | "next_week" | "nextweek" | "nw" | "w" => Some(today + Duration::days(7)),
        other => weekday_from_str(other).map(|wd| next_weekday_on_or_after(today, wd)),
    };
    target.and_then(with_default_time)
}

fn weekday_from_str(s: &str) -> Option<Weekday> {
    match s {
        "mon" | "monday" => Some(Weekday::Mon),
        "tue" | "tues" | "tuesday" => Some(Weekday::Tue),
        "wed" | "weds" | "wednesday" => Some(Weekday::Wed),
        "thu" | "thur" | "thurs" | "thursday" => Some(Weekday::Thu),
        "fri" | "friday" => Some(Weekday::Fri),
        "sat" | "saturday" => Some(Weekday::Sat),
        "sun" | "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

/// Smallest date `>= today` whose weekday is `wd`. Today counts, so `mon` on
/// Monday returns today.
fn next_weekday_on_or_after(today: NaiveDate, wd: Weekday) -> NaiveDate {
    let today_wd = today.weekday().num_days_from_monday() as i64;
    let target_wd = wd.num_days_from_monday() as i64;
    let delta = (target_wd - today_wd).rem_euclid(7);
    today + Duration::days(delta)
}

/// Attach the default time (23:59 today, 09:00 otherwise) to a naive date and
/// convert to UTC via the local timezone.
fn with_default_time(date: NaiveDate) -> Option<DateTime<Utc>> {
    let today = Local::now().date_naive();
    let time = if date == today {
        NaiveTime::from_hms_opt(23, 59, 0)?
    } else {
        NaiveTime::from_hms_opt(9, 0, 0)?
    };
    let naive = NaiveDateTime::new(date, time);
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|d| d.with_timezone(&Utc))
}

/// Reschedule a `previous` due date onto a `new_date` (local), preserving the
/// task's original time-of-day if it had one that isn't the placeholder 09:00
/// or 23:59. Falls back to the default time for that date.
pub fn reschedule_to(
    previous: Option<DateTime<Utc>>,
    new_date: NaiveDate,
) -> Option<DateTime<Utc>> {
    let default = with_default_time(new_date)?;
    let Some(prev) = previous else {
        return Some(default);
    };
    let prev_local = prev.with_timezone(&Local);
    let prev_time = prev_local.time();
    let default_local = default.with_timezone(&Local).time();
    if prev_time == NaiveTime::from_hms_opt(9, 0, 0)?
        || prev_time == NaiveTime::from_hms_opt(23, 59, 0)?
        || prev_time == default_local
    {
        return Some(default);
    }
    let naive = NaiveDateTime::new(new_date, prev_time);
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|d| d.with_timezone(&Utc))
}

/// Whitespace-tokenise `input`, pulling out `@project`, `!priority`, `^date`,
/// and `%recurrence` tokens. Anything unrecognised stays in the name; a later
/// token of the same kind overrides earlier ones so users can retype.
pub fn parse_quick_add(input: &str) -> ParsedQuickAdd {
    let mut project: Option<String> = None;
    let mut priority: Option<Priority> = None;
    let mut due: Option<DateTime<Utc>> = None;
    let mut recurrence: Option<Recurrence> = None;
    let mut kept: Vec<&str> = Vec::new();

    for token in input.split_whitespace() {
        match token.chars().next() {
            Some('@') if token.len() > 1 => {
                let body = &token[1..];
                let end = body
                    .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                    .unwrap_or(body.len());
                if end > 0 {
                    project = Some(body[..end].to_string());
                    continue;
                }
            }
            Some('!') if token.len() > 1 => {
                if let Some(p) = parse_priority_token(&token[1..]) {
                    priority = Some(p);
                    continue;
                }
            }
            Some('^') if token.len() > 1 => {
                if let Some(d) = parse_date_shortcut(&token[1..]) {
                    due = Some(d);
                    continue;
                }
            }
            Some('%') if token.len() > 1 => {
                if let Some(r) = parse_recurrence(&token[1..]) {
                    recurrence = Some(r);
                    continue;
                }
            }
            _ => {}
        }
        kept.push(token);
    }

    ParsedQuickAdd {
        name: kept.join(" ").trim().to_string(),
        project,
        priority,
        due,
        recurrence,
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
    Recurrence,
    Notes,
}

impl SheetField {
    const ORDER: [SheetField; 6] = [
        SheetField::Name,
        SheetField::Project,
        SheetField::Priority,
        SheetField::Due,
        SheetField::Recurrence,
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
    pub recurrence: String,
    pub recurrence_error: bool,
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
    /// Buffer for the `r` reschedule prompt (`InputMode::Rescheduling`).
    pub reschedule_input: String,
    /// True after a failed parse in the reschedule prompt; the UI paints red.
    pub reschedule_error: bool,
    /// Task uuids the user has marked with `v` for bulk actions. When
    /// non-empty, bulk-op keys (Space/x, d, 1/2/3) act on this set instead
    /// of the active task.
    pub marked_uuids: std::collections::BTreeSet<String>,
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
            reschedule_input: String::new(),
            reschedule_error: false,
            marked_uuids: std::collections::BTreeSet::new(),
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
                        recurrence: task
                            .recurrence
                            .as_ref()
                            .map(|r| r.to_shortcut())
                            .unwrap_or_default(),
                        recurrence_error: false,
                        notes,
                    });
                    self.input_mode = InputMode::EditingSheet;
                }
            }
        }
    }

    /// Mutable handle to the buffer of the currently focused text field, if the
    /// focused field is a plain text field (Name/Project/Due/Recurrence).
    pub fn sheet_text_field_mut(&mut self) -> Option<&mut String> {
        let sheet = self.edit_sheet.as_mut()?;
        match sheet.field {
            SheetField::Name => Some(&mut sheet.name),
            SheetField::Project => Some(&mut sheet.project),
            SheetField::Due => {
                sheet.due_error = false;
                Some(&mut sheet.due)
            }
            SheetField::Recurrence => {
                sheet.recurrence_error = false;
                Some(&mut sheet.recurrence)
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
        let recurrence = {
            let raw = sheet.recurrence.trim();
            if raw.is_empty() {
                None
            } else {
                match parse_recurrence(raw) {
                    Some(r) => Some(r),
                    None => {
                        sheet.recurrence_error = true;
                        sheet.field = SheetField::Recurrence;
                        return;
                    }
                }
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
            task.recurrence = recurrence;
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

    // --- Reschedule prompt (`r`) & one-key presets (`t`, `T`, `w`) ---

    /// Open the reschedule prompt for the active task. Returns silently if no
    /// task is selected or it is already completed.
    pub fn start_reschedule(&mut self, app: &App) {
        if let Some(idx) = app.active_task_index {
            if app.tasks.get(idx).is_some_and(|t| !t.completed) {
                self.reschedule_input.clear();
                self.reschedule_error = false;
                self.input_mode = InputMode::Rescheduling;
            }
        }
    }

    /// Attempt to parse the reschedule buffer. Empty input clears the due
    /// date; a shortcut we can't parse flags `reschedule_error` and leaves the
    /// prompt open.
    pub fn submit_reschedule(&mut self, app: &mut App) {
        let trimmed = self.reschedule_input.trim();
        if trimmed.is_empty() {
            app.set_active_due(None);
            self.reschedule_input.clear();
            self.reschedule_error = false;
            self.input_mode = InputMode::Normal;
            return;
        }
        let idx = app.active_task_index;
        let previous = idx.and_then(|i| app.tasks.get(i)).and_then(|t| t.due_date);
        match parse_date_shortcut(trimmed) {
            Some(new_due) => {
                let final_due = reschedule_to(previous, new_due.with_timezone(&Local).date_naive())
                    .unwrap_or(new_due);
                app.set_active_due(Some(final_due));
                self.reschedule_input.clear();
                self.reschedule_error = false;
                self.input_mode = InputMode::Normal;
            }
            None => self.reschedule_error = true,
        }
    }

    pub fn cancel_reschedule(&mut self) {
        self.reschedule_input.clear();
        self.reschedule_error = false;
        self.input_mode = InputMode::Normal;
    }

    /// Move the active task's due date to `target_date` local, keeping the
    /// existing time-of-day when the task already had a real due time set.
    pub fn reschedule_active(&mut self, app: &mut App, target_date: NaiveDate) {
        let idx = match app.active_task_index {
            Some(i) => i,
            None => return,
        };
        let previous = app.tasks.get(idx).and_then(|t| t.due_date);
        if let Some(new_due) = reschedule_to(previous, target_date) {
            app.set_active_due(Some(new_due));
        }
    }

    pub fn reschedule_today(&mut self, app: &mut App) {
        self.reschedule_active(app, Local::now().date_naive());
    }

    pub fn reschedule_tomorrow(&mut self, app: &mut App) {
        self.reschedule_active(app, Local::now().date_naive() + Duration::days(1));
    }

    pub fn reschedule_next_week(&mut self, app: &mut App) {
        self.reschedule_active(app, Local::now().date_naive() + Duration::days(7));
    }

    // --- Bulk-action mark set (Phase 6) ---

    /// Toggle the mark on the active task. No-op when there is no active task.
    pub fn toggle_mark_active(&mut self, app: &App) {
        if let Some(idx) = app.active_task_index {
            if let Some(task) = app.tasks.get(idx) {
                if !self.marked_uuids.remove(&task.uuid) {
                    self.marked_uuids.insert(task.uuid.clone());
                }
            }
        }
    }

    /// Drop every mark. Called on `V` and after any bulk action completes so
    /// the selection does not silently carry over into the next intent.
    pub fn clear_marks(&mut self) {
        self.marked_uuids.clear();
    }

    /// True if any tasks are marked — used by `handle_tasklist_input` to route
    /// keys to their bulk equivalents.
    pub fn has_marks(&self) -> bool {
        !self.marked_uuids.is_empty()
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
                let parsed = parse_quick_add(&self.current_input);
                if !parsed.name.is_empty() {
                    let priority = parsed.priority.unwrap_or(app.settings.default_priority);
                    let mut task = Task::new(parsed.name, parsed.project, priority);
                    task.due_date = parsed.due;
                    task.recurrence = parsed.recurrence;
                    app.tasks.push(task);
                    if app.active_task_index.is_none() {
                        app.active_task_index = Some(app.tasks.len() - 1);
                    }
                }
                self.current_input.clear();
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

    // --- Phase 4: quick-add parsing + reschedule presets ---

    #[test]
    fn quick_add_extracts_project_priority_and_due() {
        let parsed = parse_quick_add("Draft Q3 report @work !3 ^2026-08-01");
        assert_eq!(parsed.name, "Draft Q3 report");
        assert_eq!(parsed.project.as_deref(), Some("work"));
        assert_eq!(parsed.priority, Some(Priority::High));
        assert!(parsed.due.is_some());
    }

    #[test]
    fn quick_add_tokens_can_appear_anywhere() {
        let parsed = parse_quick_add("!2 buy @home milk");
        assert_eq!(parsed.name, "buy milk");
        assert_eq!(parsed.project.as_deref(), Some("home"));
        assert_eq!(parsed.priority, Some(Priority::Medium));
        assert!(parsed.due.is_none());
    }

    #[test]
    fn quick_add_later_token_overrides_earlier() {
        let parsed = parse_quick_add("thing !1 more !3");
        assert_eq!(parsed.name, "thing more");
        assert_eq!(parsed.priority, Some(Priority::High));
    }

    #[test]
    fn quick_add_ignores_bare_bang_and_at() {
        // `!` / `@` with no body are just punctuation, not tokens.
        let parsed = parse_quick_add("call @ 5pm !");
        assert_eq!(parsed.name, "call @ 5pm !");
        assert!(parsed.project.is_none());
        assert!(parsed.priority.is_none());
    }

    #[test]
    fn quick_add_unknown_tokens_stay_in_name() {
        // `!bogus` isn't a priority, `^blah` isn't a date — keep them visible.
        let parsed = parse_quick_add("do stuff !bogus ^blah");
        assert_eq!(parsed.name, "do stuff !bogus ^blah");
        assert!(parsed.priority.is_none());
        assert!(parsed.due.is_none());
    }

    #[test]
    fn date_shortcut_parses_named_dates() {
        assert!(parse_date_shortcut("today").is_some());
        assert!(parse_date_shortcut("tomorrow").is_some());
        assert!(parse_date_shortcut("mon").is_some());
        assert!(parse_date_shortcut("MONDAY").is_some());
        assert!(parse_date_shortcut("next-week").is_some());
        assert!(parse_date_shortcut("nw").is_some());
        assert!(parse_date_shortcut("2026-12-25").is_some());
        assert!(parse_date_shortcut("2026-12-25 08:30").is_some());
        assert!(parse_date_shortcut("garbage").is_none());
        assert!(parse_date_shortcut("").is_none());
    }

    #[test]
    fn date_shortcut_today_is_end_of_day() {
        let today = Local::now().date_naive();
        let d = parse_date_shortcut("today").unwrap().with_timezone(&Local);
        assert_eq!(d.date_naive(), today);
        assert_eq!(d.time(), NaiveTime::from_hms_opt(23, 59, 0).unwrap());
    }

    #[test]
    fn date_shortcut_tomorrow_is_9am() {
        let tomorrow = Local::now().date_naive() + Duration::days(1);
        let d = parse_date_shortcut("tomorrow")
            .unwrap()
            .with_timezone(&Local);
        assert_eq!(d.date_naive(), tomorrow);
        assert_eq!(d.time(), NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    }

    #[test]
    fn weekday_shortcut_returns_today_when_matching_current_day() {
        let today = Local::now().date_naive();
        let name = match today.weekday() {
            chrono::Weekday::Mon => "mon",
            chrono::Weekday::Tue => "tue",
            chrono::Weekday::Wed => "wed",
            chrono::Weekday::Thu => "thu",
            chrono::Weekday::Fri => "fri",
            chrono::Weekday::Sat => "sat",
            chrono::Weekday::Sun => "sun",
        };
        let d = parse_date_shortcut(name).unwrap().with_timezone(&Local);
        assert_eq!(d.date_naive(), today);
    }

    #[test]
    fn reschedule_preserves_custom_time_of_day() {
        // Task had a due time of 14:30 last week; moving it to today at date-level
        // should keep 14:30 rather than snap to a default.
        let last_week = Local::now().date_naive() - Duration::days(7);
        let naive = NaiveDateTime::new(last_week, NaiveTime::from_hms_opt(14, 30, 0).unwrap());
        let previous = Local
            .from_local_datetime(&naive)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let today = Local::now().date_naive();
        let out = reschedule_to(Some(previous), today)
            .unwrap()
            .with_timezone(&Local);
        assert_eq!(out.date_naive(), today);
        assert_eq!(out.time(), NaiveTime::from_hms_opt(14, 30, 0).unwrap());
    }

    #[test]
    fn reschedule_uses_default_when_previous_had_default_time() {
        // Task previously set via `^today` (23:59). Rescheduling to tomorrow
        // should snap to 09:00, not carry the 23:59 marker across.
        let today = Local::now().date_naive();
        let naive = NaiveDateTime::new(today, NaiveTime::from_hms_opt(23, 59, 0).unwrap());
        let previous = Local
            .from_local_datetime(&naive)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        let tomorrow = today + Duration::days(1);
        let out = reschedule_to(Some(previous), tomorrow)
            .unwrap()
            .with_timezone(&Local);
        assert_eq!(out.time(), NaiveTime::from_hms_opt(9, 0, 0).unwrap());
    }

    #[test]
    fn submit_task_applies_quick_add_tokens_to_new_task() {
        let mut app = App::default();
        let mut ui = UiState {
            current_input: "buy milk @home !3 ^tomorrow".into(),
            ..UiState::default()
        };
        ui.submit_task(&mut app);
        assert_eq!(app.tasks.len(), 1);
        let t = &app.tasks[0];
        assert_eq!(t.name, "buy milk");
        assert_eq!(t.project.as_deref(), Some("home"));
        assert_eq!(t.priority, Priority::High);
        assert!(t.due_date.is_some());
    }

    #[test]
    fn reschedule_today_sets_due_on_task_with_no_previous_date() {
        let mut app = App::default();
        app.tasks
            .push(Task::new("thing".into(), None, Priority::Medium));
        app.active_task_index = Some(0);
        let mut ui = UiState::default();
        ui.reschedule_today(&mut app);
        let due = app.tasks[0].due_date.expect("t set a due date");
        assert_eq!(
            due.with_timezone(&Local).date_naive(),
            Local::now().date_naive()
        );
    }

    #[test]
    fn submit_reschedule_with_empty_input_clears_due() {
        let mut app = App::default();
        let mut task = Task::new("thing".into(), None, Priority::Medium);
        task.due_date = Some(Utc::now() + Duration::days(1));
        app.tasks.push(task);
        app.active_task_index = Some(0);
        let mut ui = UiState::default();
        ui.start_reschedule(&app);
        ui.submit_reschedule(&mut app);
        assert!(
            app.tasks[0].due_date.is_none(),
            "empty input clears due date"
        );
    }

    #[test]
    fn submit_reschedule_bad_input_flags_error_and_keeps_prompt_open() {
        let mut app = App::default();
        app.tasks
            .push(Task::new("t".into(), None, Priority::Medium));
        app.active_task_index = Some(0);
        let mut ui = UiState::default();
        ui.start_reschedule(&app);
        ui.reschedule_input = "not a date".into();
        ui.submit_reschedule(&mut app);
        assert!(ui.reschedule_error);
        assert!(matches!(ui.input_mode, InputMode::Rescheduling));
        assert!(app.tasks[0].due_date.is_none());
    }

    // ------------------------------------------------------------------
    // Phase 5: recurring tasks
    // ------------------------------------------------------------------

    #[test]
    fn parse_recurrence_named_forms() {
        assert_eq!(parse_recurrence("daily"), Some(Recurrence::EveryDays(1)));
        assert_eq!(parse_recurrence("weekly"), Some(Recurrence::EveryWeeks(1)));
        assert_eq!(
            parse_recurrence("monthly"),
            Some(Recurrence::EveryMonths(1))
        );
    }

    #[test]
    fn parse_recurrence_shorthand_units() {
        assert_eq!(parse_recurrence("2d"), Some(Recurrence::EveryDays(2)));
        assert_eq!(parse_recurrence("3w"), Some(Recurrence::EveryWeeks(3)));
        assert_eq!(parse_recurrence("6m"), Some(Recurrence::EveryMonths(6)));
    }

    #[test]
    fn parse_recurrence_weekday_names() {
        assert_eq!(
            parse_recurrence("mon"),
            Some(Recurrence::Weekly(Weekday::Mon))
        );
        assert_eq!(
            parse_recurrence("Friday"),
            Some(Recurrence::Weekly(Weekday::Fri))
        );
    }

    #[test]
    fn parse_recurrence_rejects_garbage() {
        assert_eq!(parse_recurrence(""), None);
        assert_eq!(parse_recurrence("0d"), None);
        assert_eq!(parse_recurrence("garbage"), None);
        assert_eq!(parse_recurrence("2x"), None);
    }

    #[test]
    fn quick_add_extracts_recurrence_token() {
        let p = parse_quick_add("water plants %2d @home");
        assert_eq!(p.name, "water plants");
        assert_eq!(p.recurrence, Some(Recurrence::EveryDays(2)));
        assert_eq!(p.project.as_deref(), Some("home"));
    }

    #[test]
    fn quick_add_unknown_recurrence_token_stays_in_name() {
        let p = parse_quick_add("read %bogus");
        assert_eq!(p.name, "read %bogus");
        assert_eq!(p.recurrence, None);
    }

    #[test]
    fn next_after_every_days_advances_by_interval() {
        let base = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
        assert_eq!(
            Recurrence::EveryDays(3).next_after(base),
            base + Duration::days(3)
        );
    }

    #[test]
    fn next_after_every_weeks_advances_by_seven_days_per_unit() {
        let base = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
        assert_eq!(
            Recurrence::EveryWeeks(2).next_after(base),
            base + Duration::days(14)
        );
    }

    #[test]
    fn next_after_every_months_handles_end_of_month() {
        // Jan 31 + 1 month = Feb 29 in a leap year (chrono clamps).
        let base = Utc.with_ymd_and_hms(2024, 1, 31, 9, 0, 0).unwrap();
        let next = Recurrence::EveryMonths(1).next_after(base);
        assert_eq!(next.date_naive().month(), 2);
        assert_eq!(next.date_naive().day(), 29);
    }

    #[test]
    fn next_after_weekly_jumps_seven_days_when_already_on_target() {
        // 2024-01-01 was a Monday.
        let monday = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
        let next = Recurrence::Weekly(Weekday::Mon).next_after(monday);
        assert_eq!(next, monday + Duration::days(7));
    }

    #[test]
    fn next_after_weekly_snaps_to_next_target_weekday() {
        // 2024-01-02 was a Tuesday; next Friday is 2024-01-05.
        let tuesday = Utc.with_ymd_and_hms(2024, 1, 2, 9, 0, 0).unwrap();
        let next = Recurrence::Weekly(Weekday::Fri).next_after(tuesday);
        assert_eq!(next.date_naive().weekday(), Weekday::Fri);
        assert_eq!(next - tuesday, Duration::days(3));
    }

    #[test]
    fn completing_recurring_task_spawns_next_occurrence() {
        let mut app = App::default();
        let mut task = Task::new("water".into(), None, Priority::Medium);
        task.due_date = Some(Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap());
        task.recurrence = Some(Recurrence::EveryDays(2));
        task.subtasks.push(SubTask::new("sub".into()));
        task.subtasks[0].done = true;
        app.tasks.push(task);
        app.active_task_index = Some(0);

        app.complete_active_task();

        assert_eq!(app.tasks.len(), 2, "spawns a fresh occurrence");
        assert!(app.tasks[0].completed, "original marked done");
        let spawned = &app.tasks[1];
        assert!(!spawned.completed);
        assert_eq!(spawned.name, "water");
        assert_eq!(spawned.recurrence, Some(Recurrence::EveryDays(2)));
        assert_eq!(
            spawned.due_date,
            Some(Utc.with_ymd_and_hms(2024, 1, 3, 9, 0, 0).unwrap())
        );
        assert_eq!(spawned.subtasks.len(), 1);
        assert!(!spawned.subtasks[0].done, "subtask done flag reset");
        assert_ne!(spawned.uuid, app.tasks[0].uuid, "fresh uuid");
        assert_eq!(
            app.active_task_index,
            Some(1),
            "cursor lands on new occurrence"
        );
    }

    #[test]
    fn completing_non_recurring_task_does_not_spawn() {
        let mut app = App::default();
        app.tasks
            .push(Task::new("one-off".into(), None, Priority::Medium));
        app.active_task_index = Some(0);
        app.complete_active_task();
        assert_eq!(app.tasks.len(), 1);
        assert!(app.tasks[0].completed);
    }

    #[test]
    fn edit_sheet_seeds_and_saves_recurrence() {
        let mut app = App::default();
        let mut task = Task::new("rec".into(), None, Priority::Medium);
        task.recurrence = Some(Recurrence::EveryWeeks(2));
        app.tasks.push(task);
        app.active_task_index = Some(0);

        let mut ui = UiState::default();
        ui.open_edit_sheet(&app);
        let sheet = ui.edit_sheet.as_ref().expect("sheet open");
        assert_eq!(sheet.recurrence, "2w");

        // Clear it via the edit sheet and confirm the task drops recurrence.
        ui.edit_sheet.as_mut().unwrap().recurrence.clear();
        ui.submit_sheet(&mut app);
        assert_eq!(app.tasks[0].recurrence, None);
    }

    #[test]
    fn edit_sheet_bad_recurrence_keeps_sheet_open() {
        let mut app = App::default();
        app.tasks
            .push(Task::new("rec".into(), None, Priority::Medium));
        app.active_task_index = Some(0);

        let mut ui = UiState::default();
        ui.open_edit_sheet(&app);
        ui.edit_sheet.as_mut().unwrap().recurrence = "nonsense".into();
        ui.submit_sheet(&mut app);
        assert!(ui.edit_sheet.is_some(), "sheet stays open on bad input");
        let sheet = ui.edit_sheet.as_ref().unwrap();
        assert!(sheet.recurrence_error);
        assert_eq!(sheet.field, SheetField::Recurrence);
        assert_eq!(app.tasks[0].recurrence, None);
    }

    // ------------------------------------------------------------------
    // Phase 6: bulk actions
    // ------------------------------------------------------------------

    fn app_with_three() -> App {
        let mut app = App::default();
        app.tasks.push(Task::new("one".into(), None, Priority::Low));
        app.tasks.push(Task::new("two".into(), None, Priority::Low));
        app.tasks
            .push(Task::new("three".into(), None, Priority::Low));
        app.active_task_index = Some(0);
        app
    }

    #[test]
    fn toggle_mark_adds_then_removes_active_task() {
        let app = app_with_three();
        let mut ui = UiState::default();
        let uuid0 = app.tasks[0].uuid.clone();
        ui.toggle_mark_active(&app);
        assert!(ui.marked_uuids.contains(&uuid0));
        assert!(ui.has_marks());
        ui.toggle_mark_active(&app);
        assert!(!ui.marked_uuids.contains(&uuid0));
        assert!(!ui.has_marks());
    }

    #[test]
    fn clear_marks_wipes_selection() {
        let mut app = app_with_three();
        let mut ui = UiState::default();
        ui.toggle_mark_active(&app);
        app.active_task_index = Some(1);
        ui.toggle_mark_active(&app);
        assert_eq!(ui.marked_uuids.len(), 2);
        ui.clear_marks();
        assert!(ui.marked_uuids.is_empty());
    }

    #[test]
    fn bulk_complete_marks_all_selected_done() {
        let mut app = app_with_three();
        let mut marks = std::collections::BTreeSet::new();
        marks.insert(app.tasks[0].uuid.clone());
        marks.insert(app.tasks[2].uuid.clone());
        app.bulk_complete(&marks);
        assert!(app.tasks[0].completed);
        assert!(!app.tasks[1].completed);
        assert!(app.tasks[2].completed);
    }

    #[test]
    fn bulk_complete_spawns_next_occurrence_for_recurring_tasks() {
        let mut app = app_with_three();
        app.tasks[1].recurrence = Some(Recurrence::EveryDays(1));
        let mut marks = std::collections::BTreeSet::new();
        marks.insert(app.tasks[1].uuid.clone());
        app.bulk_complete(&marks);
        assert_eq!(app.tasks.len(), 4, "spawned one new occurrence");
        assert!(app.tasks[1].completed, "original done");
        assert!(!app.tasks[2].completed, "spawned copy is open");
        assert_eq!(app.tasks[2].name, "two");
    }

    #[test]
    fn bulk_delete_removes_all_selected_tasks() {
        let mut app = app_with_three();
        let mut marks = std::collections::BTreeSet::new();
        marks.insert(app.tasks[0].uuid.clone());
        marks.insert(app.tasks[2].uuid.clone());
        app.bulk_delete(&marks);
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].name, "two");
    }

    #[test]
    fn bulk_set_priority_updates_only_marked_open_tasks() {
        let mut app = app_with_three();
        app.tasks[2].completed = true; // completed tasks must not change
        let mut marks = std::collections::BTreeSet::new();
        marks.insert(app.tasks[0].uuid.clone());
        marks.insert(app.tasks[2].uuid.clone());
        app.bulk_set_priority(&marks, Priority::High);
        assert_eq!(app.tasks[0].priority, Priority::High);
        assert_eq!(app.tasks[1].priority, Priority::Low, "unmarked untouched");
        assert_eq!(app.tasks[2].priority, Priority::Low, "completed untouched");
    }
}
