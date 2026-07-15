# Deferred feature: Subtasks / checklists

Status: **IMPLEMENTED** (2026-07-15).

## Decisions made during implementation

- **Parent key** — added a persistent `uuid` to `Task` (serde default + SQLite column, migrated via `ALTER TABLE`). Subtasks live in a proper `subtasks` table keyed by `task_uuid`. Both tables are still rewritten wholesale on save; uuid keeps parent identity stable across the rewrite.
- **Expand/collapse** — no persistent set. The active/selected parent auto-expands to show its checklist; all others stay collapsed. Simplest model that satisfies "expand the selected task."
- **Archive** — computed on the fly from `completion_date` (`done` for > 24h); no stored flag, so it self-updates every render/tick. Archived subtasks are hidden by default, revealed globally with `Shift+A`, never deleted.
- **Parent completion** — `Enter` still toggles the parent independently of subtask state (no auto-complete / no warning). Kept the existing behaviour to avoid surprising the user.
- **Dashboard counter** — left unchanged (counts parent tasks; subtask state does not affect it).
- **Nav** — `selected_subtask: Option<usize>` under the active parent; `j`/`k` step in and out. `Space`/`x` toggles the highlighted subtask.

## Goal

Let a task hold a checklist of subtasks that can be added and ticked off inline — without leaving the Task List view and without the parent being treated as done until the user says so.

## Requirements

1. **Data model** — add to `Task` (`src/app/mod.rs`):
   ```rust
   pub struct SubTask {
       pub name: String,
       pub done: bool,
       pub creation_date: DateTime<Utc>,
       pub completion_date: Option<DateTime<Utc>>,
   }
   // on Task:
   pub subtasks: Vec<SubTask>,   // #[serde(default)]
   ```
   Persist in SQLite: new table `subtasks(id, task_id, sort_order, name, done, creation_date, completion_date)` keyed by parent, or a JSON blob column on `tasks`. Prefer a proper table with a stable parent key (currently tasks are rewritten wholesale on save — revisit `save_tasks` so parent identity survives, e.g. add a persistent `uuid` to `Task`).

2. **Inline add** — in Task List, a key (suggest `a`) opens an inline input under the selected parent to add a subtask. Reuse the `InputMode::Editing` pattern; add `InputMode::EditingSubtask` + `editing_subtask_parent: Option<usize>` to `UiState`.

3. **Inline check** — expand the selected task to show its subtasks; a key (suggest `Space` or `x`) toggles the highlighted subtask's `done`. Navigation must step into subtask rows. Consider a flattened render model: `Vec<Row { parent_idx, sub_idx: Option<usize> }>`.

4. **Progress indicator** — show `[2/5]` next to a parent that has subtasks; optionally a mini gauge.

5. **Auto-archive of done subtasks** — a subtask that has been `done` for **> 24h** moves to a collapsed "archived" section under its parent (still visible on demand, e.g. toggle with `Shift+A`), so the active checklist stays short but nothing is lost. Compute on load + on each tick using `completion_date`. Do **not** delete — only hide from the default view.

6. **Parent completion** — completing a parent (Enter) should still work independently of subtask state, but consider: offer to auto-complete remaining subtasks, or warn if subtasks are open. Decide during implementation.

## Open questions to resolve before building

- One flat expand/collapse per task, or a persistent "expanded" set?
- Does completing all subtasks auto-suggest completing the parent?
- Should the dashboard "open tasks" counter include or exclude tasks whose subtasks are all done?

## Touch points

- `src/app/mod.rs` — `Task`, `SubTask`, methods (add/toggle/archive subtask).
- `src/app/ui_state.rs` — new input mode + navigation over flattened rows.
- `src/db.rs` — schema + load/save for subtasks (stable parent key needed).
- `src/ui/task_list.rs` — expanded rendering, progress badge, archived section.
- `src/main.rs` — key routing for add/toggle/expand; tick-time archive sweep.
