# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```shell
cargo run                          # launch the TUI (takes over the terminal via alternate screen)
cargo build --release              # optimized build (opt-level=z, lto, panic=abort)
cargo test                         # run all tests
cargo test subtasks_round_trip     # run a single test by name
cargo clippy                       # lint
cargo fmt                          # format
```

Tests live inline as `#[cfg(test)] mod tests` (see `src/db.rs`) and use temp-dir SQLite files keyed by process id, so they run without touching the user's real database.

## Architecture

Terminal to-do manager built on `ratatui` + `crossterm`. Single-threaded event loop, no async.

**Event loop (`src/main.rs`).** `run_app` draws every frame then polls for input on a 250 ms tick. Each tick fires `check_due_notifications` (one desktop notification per task the moment its due date passes, gated on `due_notified`) and autosaves every 120 ticks (~30 s). A panic hook restores the terminal (leaves alternate screen, disables raw mode) before propagating any panic — keep it intact when editing terminal setup.

**Input dispatch.** All key handling is centralized in `main.rs::handle_key_event`, which branches first on `UiState::input_mode` (`Normal`, `Editing`, `Filtering`, `EditingNotes`, `EditingDue`, `EditingSubtask`), then in `Normal` mode on `app.current_view`. Each `View` has its own `handle_*_input` fn. Adding a keybinding means editing the matching handler here, not the UI module.

**State split — this is the core design.**
- `App` (`src/app/mod.rs`) = persisted domain state: `tasks: Vec<Task>`, `sort_mode`, `current_view`, `active_task_index`, plus non-persisted `settings` and `should_quit`. Serializable; all task mutations (complete/delete/reorder/priority) are methods here.
- `UiState` (`src/app/ui_state.rs`) = transient view state, never persisted: input buffers, textareas, selection cursors, modal edit targets. UI methods that start/submit/cancel an edit (e.g. `start_edit_due`, `submit_subtask`) live here and reach into `App` to apply changes.

The rendering layer (`src/ui/*`) is pure: each `draw_*` fn reads `App`/`UiState`/`Theme` and produces widgets. It never mutates. `ui()` in `main.rs` selects the view draw fn and overlays modals.

**Tasks and subtasks.** `Task` carries a stable `uuid` (serde default `new_uuid`) so subtasks stay keyed to their parent across the wholesale table rewrite on save. Subtasks auto-archive 24 h after completion (`SUBTASK_ARCHIVE_AFTER`, `SubTask::is_archived`); `Task::visible_subtask_indices` maps the display order (active first, archived only when `show_archived`) that the UI navigates. Task ordering for display comes from `App::ordered_active_indices` / `ordered_completed_indices` (respect `sort_mode`, apply filter), not from raw `tasks` order.

**Persistence (`src/db.rs`).** SQLite via `rusqlite` (bundled, WAL mode) at `~/.local/share/todorust/todorust.db` (path from `directories::ProjectDirs`). `save_to` runs in one transaction and does a full DELETE + reinsert of `tasks` and `subtasks` — `sort_order` columns preserve `Vec` order. `app_state` is a key/value table for view/sort/active-index. `init_schema` is additive-only: schema changes go through `CREATE TABLE IF NOT EXISTS` plus tolerated `ALTER TABLE ... ADD COLUMN` (see the uuid migration), and legacy rows without a uuid get one minted on load. On first run with no db, `load_with_settings` migrates a legacy `state.json` if present.

**Settings & themes.** Settings persist separately as TOML at `~/.config/todorust/config.toml` (not in the db) via a `SerializableSettings` mirror. `Theme::from_settings` maps the `ColorTheme` enum to a concrete `Theme`; `Custom` reads `[custom_theme]` from config, each unset field falling back to Default.

**Project tags.** `@tag` syntax in a task name is parsed out by `parse_project` (in `ui_state.rs`) into a separate `project` field; `task_matches_filter` searches name, notes, and project.
