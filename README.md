![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/License-CC%20%7C%20BY--NC--SA%204.0-green)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-blue)
![Rust Version](https://img.shields.io/badge/rust-1.88.0-blue)

# Todorust ✓

A minimalist, powerful, terminal-based to-do manager written in Rust. Fast to capture,
organised by section, and safe to use daily.

Inspired by [pomodorust](https://github.com/ruteckimikolaj/pomodorust) — its sibling
terminal Pomodoro timer — Todorust shares that project's look, feel, and design
principles: `@tag` projects, real-time `/` search, weekly statistics, six built-in
themes with a `[custom_theme]` override, native desktop notifications, and SQLite
persistence.

## ✨ Features

- **Quick-add capture** — Type `Draft Q3 report @work !3 ^fri %weekly` and every token
  (project, priority, due date, recurrence) is parsed into structured fields as you go,
  with a live preview above the input.
- **Grouped agenda** — The Task List is split into sections (Overdue / Today / Upcoming /
  No date, or by project or priority). Cycle with `g`. A compact **Today strip** at the
  top of the screen shows overdue/today counts at a glance.
- **Single edit sheet** — `Enter` or `e` opens one modal that edits every attribute of a
  task (name, project, priority, due, recurrence, notes). Retires per-attribute keys.
- **Reschedule presets** — `t` = today, `T` = tomorrow, `w` = next week, `r` opens a
  prompt (`mon`..`sun`, `YYYY-MM-DD`, empty = clear). The task's time-of-day is preserved
  when it wasn't a placeholder.
- **Recurring tasks** — `daily`, `weekly`, `monthly`, `2d` / `3w` / `1m`, or a weekday
  name. Completing a recurring task instantly spawns the next occurrence with subtask
  flags reset.
- **Subtasks** — Add with `+`; done subtasks auto-archive after 24 h. Progress badge
  `[2/5]` next to the parent.
- **Bulk actions** — `v` marks the current task (`[•]` in the row); Space/x, d, and
  1/2/3 then apply to the whole marked set. `Shift+V` clears marks.
- **Priority gutter** — Priority is shown as a coloured left bar in every row for
  fast scanning.
- **Filter / search** — `/` narrows by name, notes, or project in real time.
- **Statistics + Review** — Weekly bar chart, per-day sparkline on narrow terminals,
  plus a review panel with completion rate, average age of open tasks, week-vs-week
  trend, and top open projects.
- **Delete with confirm** — A single keystroke never destroys data; `d` arms a `y/N`
  prompt.
- **Desktop notifications** — Native alerts when a task becomes due.
- **Mouse support** — Wheel scrolls up/down through the current list.
- **Import / Export JSON** — `todorust export tasks.json` and
  `todorust import tasks.json [--replace]`. Use `-` for stdin/stdout to pipe.
- **SQLite persistence** — Tasks and app state in a local SQLite database
  (`~/.local/share/todorust/todorust.db`). Settings persist separately as TOML
  (`~/.config/todorust/config.toml`).
- **Six built-in themes** — Default, Dracula, Solarized, Nord, Gruvbox Dark,
  Cyberpunk — plus a `[custom_theme]` config block.
- **Cross-platform** — Runs on macOS and Linux.

## 📦 Installation

### Using Cargo

```shell
cargo install todorust
```

### Using Homebrew

```shell
brew tap ruteckimikolaj/tap
brew install todorust
```

### Download a release binary

Prebuilt binaries for `linux-amd64`, `linux-arm64`, `darwin-amd64`, and `darwin-arm64`
are attached to each [GitHub Release](https://github.com/ruteckimikolaj/todorust/releases).

## 🚀 Usage

```shell
todorust                         # launch the TUI
todorust export tasks.json       # dump all tasks as JSON
todorust import tasks.json       # append tasks from JSON
todorust import backup.json --replace   # overwrite the store
```

The bottom bar shows the essential keys; press `?` at any time in the Task List for a
full keybinding overlay.

### Quick-add tokens

Type a task then any combination of these tokens; the parser strips them out and applies
them to the new task.

| Token | Meaning | Examples |
| ----- | ------- | -------- |
| `@name` | Project tag | `@work`, `@home` |
| `!n` | Priority | `!1`/`!low`, `!2`/`!med`, `!3`/`!high` |
| `^date` | Due date | `^today`, `^tomorrow`, `^fri`, `^nw`, `^2026-07-20`, `^2026-07-20 17:00` |
| `%repeat` | Recurrence | `%daily`, `%weekly`, `%monthly`, `%2d`, `%3w`, `%1m`, `%mon`..`%sun` |

Example: `Draft Q3 report @work !3 ^fri %weekly`.

### Global keys

| Key | Action |
| --- | ------ |
| `Tab` | Switch view (Task List ↔ Statistics) |
| `o` | Open settings panel |
| `q` / `Ctrl+C` | Quit |

### Task List

| Key | Action |
| --- | ------ |
| `↑` / `k`, `↓` / `j` | Move selection (mouse wheel also works) |
| `a` | Add task (accepts quick-add tokens) |
| `+` | Add subtask to the selected task |
| `Enter` / `e` | Open the edit sheet (all attributes) |
| `Space` / `x` | Toggle done (task or highlighted subtask, or every marked task) |
| `1` / `2` / `3` | Set priority Low / Medium / High (bulk when marks exist) |
| `t` / `T` / `w` | Reschedule to today / tomorrow / next week |
| `r` | Reschedule prompt |
| `g` (or `s`) | Cycle grouping (Smart / Project / Priority / Manual) |
| `K` / `J` (or `Shift+↑`/`↓`) | Reorder task (Manual grouping only) |
| `v` | Toggle mark on active task for bulk actions |
| `Shift+V` | Clear all marks |
| `Shift+A` | Show / hide archived subtasks (done > 24 h) |
| `/` | Filter / search (name, notes, project) |
| `d` / `Delete` | Delete (with `y/N` confirmation; bulk when marks exist) |
| `?` | Full help overlay |
| `Esc` | Cancel input / clear filter |

### Statistics

| Key | Action |
| --- | ------ |
| `↑` / `k`, `↓` / `j` | Navigate completed tasks (mouse wheel too) |
| `/` | Filter completed tasks |
| `Enter` | View task details |
| `d` / `Delete` | Delete selected task |
| `Tab` | Back to Task List |

### Settings

| Key | Action |
| --- | ------ |
| `↑` / `k`, `↓` / `j` | Select setting |
| `←` / `h`, `→` / `l` | Change value |
| `Tab` / `o` / `Esc` | Close settings |

## 📁 Data & Config Locations

| File | Purpose |
| ---- | ------- |
| `~/.local/share/todorust/todorust.db` | Tasks and app state (SQLite) |
| `~/.config/todorust/config.toml` | Theme, priority, and notification settings |

## 🎨 Custom Theme

Add a `[custom_theme]` table to `~/.config/todorust/config.toml`. All fields are
optional hex strings — omit any to inherit from the Default theme.

```toml
[custom_theme]
high_color   = "#ff2d78"
medium_color = "#ff6d00"
low_color    = "#00fff9"
done_color   = "#39ff14"
high_bg      = "#2d0018"
medium_bg    = "#281000"
low_bg       = "#002028"
accent_color = "#ffe600"
base_fg      = "#e2d9f3"
base_bg      = "#0d0221"
highlight_bg = "#1e0a3c"
help_text_fg = "#7b68ee"
```

## ❤️ Contributing

Contributions, bug reports, and feature suggestions are welcome.

1. Fork the repository.
2. Create a new branch (`git checkout -b feature/your-feature`).
3. Make your changes and commit (`git commit -m 'Add feature'`).
4. Push and open a Pull Request.
