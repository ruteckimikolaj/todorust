![Version](https://img.shields.io/badge/version-0.1.0-blue)
![https://spdx.org/licenses/CC-BY-NC-SA-4.0.json](https://img.shields.io/badge/License-CC%20%7C%20BY--NC--SA%204.0-green)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-blue)
![Rust Version](https://img.shields.io/badge/rust-1.70.0-blue)

# Todorust ✓

A minimalist, powerful, terminal-based to-do manager written in Rust to help you stay organized and get things done. Shares its look and feel with [pomodorust](https://github.com/ruteckimikolaj/pomodorust).

## ✨ Features

- **Task Management** — Create, rename, reorder, complete, and delete tasks. Assign tasks to projects using `@tag` syntax.
- **Priorities** — Every task has a Low / Medium / High priority with a coloured glyph. Cycle it with `p`.
- **Due Dates** — Set a date + time on any task (`Shift+D`). Overdue tasks are highlighted and trigger a desktop notification.
- **Sorting** — Toggle the active list between Manual, Priority, and Due Date order with `s`.
- **Dashboard** — A big block-art counter of open tasks plus today's overdue / due / completed totals.
- **Task Notes** — Attach multi-line notes to any task. Edit with a full-screen modal editor (`Shift+E`).
- **Search & Filter** — Press `/` to filter tasks by name, notes, or project tag in real time.
- **Statistics** — Weekly completions bar chart, daily and all-time summary, and a searchable history of completed tasks with per-task details.
- **Six Built-in Color Themes** — Default, Dracula, Solarized, Nord, Gruvbox Dark, Cyberpunk. Switchable from the settings panel with `←`/`→`.
- **Custom Theme** — Define your own colors in `~/.config/todorust/config.toml` under `[custom_theme]`. Any unset field falls back to the Default theme.
- **Desktop Notifications** — Native notifications when a task becomes due.
- **SQLite Persistence** — Tasks and app state stored in a local SQLite database (`~/.local/share/todorust/todorust.db`). Settings persist separately as TOML (`~/.config/todorust/config.toml`).
- **Cross-Platform** — Runs on macOS and Linux.

## 📦 Installation

### Using Cargo

```shell
cargo install todorust
```

### Using Homebrew

```shell
brew tap ruteckimikolaj/homebrew-tap
brew install todorust
```

## 🚀 Usage

```shell
todorust
```

Controls are context-sensitive and shown at the bottom of each view.

**Global**

| Key | Action |
| --- | ------ |
| `o` | Open settings panel |
| `q` | Quit |
| `Tab` | Cycle views (Tasks → Statistics → Dashboard) |

**Task List**

| Key | Action |
| --- | ------ |
| `↑` / `k`, `↓` / `j` | Navigate tasks and, within the selected task, its subtasks |
| `Shift+↑` / `K`, `Shift+↓` / `J` | Reorder selected task (Manual sort only) |
| `n` | New task (supports `@project` tag, e.g. `Buy milk @work`) |
| `a` | Add a subtask to the selected task |
| `Space` / `x` | Toggle the highlighted subtask done |
| `Shift+A` | Show / hide archived subtasks (done > 24h) |
| `e` | Rename selected task |
| `p` | Cycle priority (Low → Medium → High) |
| `Shift+D` | Set / clear due date (`YYYY-MM-DD HH:MM`) |
| `s` | Cycle sort mode (Manual → Priority → Due Date) |
| `Shift+E` | Edit notes for selected task |
| `Enter` | Toggle task complete / incomplete |
| `d` / `Delete` | Delete selected task |
| `/` | Enter filter mode — narrow by name, notes, or `@project` |
| `Esc` | Clear filter / cancel input |

**Statistics**

| Key | Action |
| --- | ------ |
| `↑` / `k`, `↓` / `j` | Navigate completed tasks |
| `/` | Filter completed tasks |
| `Enter` | View task details |
| `d` / `Delete` | Delete selected task |

**Settings**

| Key | Action |
| --- | ------ |
| `↑` / `k`, `↓` / `j` | Select setting |
| `←` / `h`, `→` / `l` | Change value |
| `Tab` | Close settings |

### Projects

Append `@tag` anywhere in a task name to assign it to a project:

```
Write report @work
Buy groceries @personal
```

The tag is stripped from the display name and shown as a coloured badge. Filter by `@work` or just `work`.

### Data & Config Locations

| File | Purpose |
| ---- | ------- |
| `~/.local/share/todorust/todorust.db` | Tasks and app state (SQLite) |
| `~/.config/todorust/config.toml` | Theme, priority, and notification settings |

### Custom Theme

Add a `[custom_theme]` table to `~/.config/todorust/config.toml`. All fields are optional hex strings — omit any to inherit from the Default theme.

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
