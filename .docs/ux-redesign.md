# Follow-up: UX redesign for real to-do workflow

Status: **PLANNING**. The v0.1 foundation (ratatui shell, SQLite, themes, filtering,
priority, due dates, sort) works, but it is still shaped like the Pomodoro app it was
forked from. This document lists best-in-class options and a concrete redesign to make
todorust feel like a first-class terminal to-do manager (reference points: Things 3,
Todoist, TaskWarrior, Superlist, org-mode/org-agenda).

The two hard requirements driving this:

1. **It must fit a to-do workflow flawlessly** — capture fast, triage, schedule, do, review.
2. **The Task List control bar is overloaded** — 13 keys today, they overflow the screen and
   aren't intuitive. Collapse to an essential, memorable set.

---

## 1. Problem inventory (current v0.1)

- **Control bar overflow.** `[Tab][↑/↓][S+↑/↓][n][e][p][D][E][s][/][Ent][d][q]` — 13 chords.
  Wide help string is ~150 cols; it truncates on any normal terminal. One key per attribute
  (`e` rename, `p` priority, `D` due, `Shift+E` notes) does not scale.
- **Non-intuitive mappings.** `Enter` = toggle complete (users expect Enter = open). `d` = delete
  (destructive on a single unmodified keypress, no confirm/undo). `Shift+E`/`Shift+D` are awkward.
- **No structure to the list.** A to-do app lives on *grouping*: Overdue / Today / Upcoming /
  No-date / Done. Right now it's one flat list; sort is a global toggle, not a view.
- **Dashboard is decorative.** Big number is nice but not actionable — the "home" screen of a
  to-do app should be the **Today agenda**, not a counter.
- **Capture is slow.** Setting project/priority/due each needs a separate mode after creation.
  Best-in-class apps parse it all from one quick-add line.
- **Details are read-only and completed-only.** No single place to edit everything about a task.
- **No undo, no confirm, no recurring tasks, no reschedule presets.**

---

## 2. Task List control redesign (the priority ask)

### Principle
One key per *verb*, not per *attribute*. Attributes are edited in one place (an edit sheet).
Anything non-essential moves to a `?` help overlay so the bottom bar stays short.

### Essential bar (target: ≤ 8 items, fits 60 cols)
```
 [a]dd  [Space] done  [e]dit  [d]el  [/] find  [g]roup  [Tab] views  [?] help
```

### Full mapping — current → proposed

| Action | Current | Proposed | Rationale |
|---|---|---|---|
| Navigate | `↑/↓ j/k` | `↑/↓ j/k` | keep |
| Toggle done | `Enter` | `Space` (or `x`) | Space = the checkbox; frees Enter |
| Open / edit task | — | `Enter` **or** `e` | opens the **edit sheet** (see §3.4) |
| Add task | `n` | `a` | `a`dd is the near-universal verb |
| Quick-add w/ syntax | — | `a` then type `Buy milk @home !1 ^fri 5pm` | one-line capture (see §4.1) |
| Rename | `e` | (folded into edit sheet) | remove standalone |
| Priority | `p` (cycle) | inside edit sheet **+** `1`/`2`/`3` fast-set on selection | direct-set beats cycle |
| Due date | `Shift+D` | inside edit sheet **+** `t`/`T` today/tomorrow, `r` reschedule | presets are best-in-class |
| Notes | `Shift+E` | inside edit sheet (`n` field / opens modal) | remove standalone chord |
| Sort | `s` | `g` cycles **grouping** (see §3.1); manual reorder via `K`/`J` | grouping replaces sort |
| Reorder | `Shift+↑/↓` `K/J` | `K`/`J` (Manual group only) | keep, drop the arrow variant |
| Delete | `d` / `Delete` | `d` **with confirm or undo** | never silent-destroy |
| Filter/search | `/` | `/` | keep |
| Settings | `o` | `o` | keep |
| Help overlay | — | `?` | absorbs everything rare |
| Quit | `q` | `q` | keep |

Net: bottom bar goes from 13 → ~8 chords; every rare action lives behind `?` or the edit sheet.

### Alternatives considered
- **Leader-key (vim `<space>` menu / which-key popup).** Very scalable, discoverable via popup.
  Recommended as a *phase 2* enhancement layered on top of the essential bar.
- **Command palette (`:` / Ctrl-P fuzzy actions).** Great for power users; add later, not a
  replacement for direct keys.

---

## 3. View redesigns

### 3.1 Task List → grouped agenda (biggest win)
Replace the flat active list + global sort with **section grouping**, `g` cycles the grouping mode:

- **Smart (default):** `⚠ Overdue`, `● Today`, `↗ Upcoming`, `◦ No date`, collapsed `✓ Done today`.
- **By project:** one section per `@project`.
- **By priority:** High / Medium / Low.
- **Manual:** flat, user-ordered (enables `K`/`J` reorder).

Rendering: section headers as dim bold rows with counts (`Today  3`); navigation skips headers.
Collapsible sections (`h`/`l` or `Enter` on a header) — keep long lists manageable.

Best-in-class references: Things "Today/Upcoming", org-agenda day view, Todoist smart lists.

### 3.2 Dashboard → Today / Agenda home
Make the default landing view an **actionable Today agenda**, not a counter:
- Top: date + a compact stat strip (open / overdue / due today / done today).
- Body: today's + overdue tasks, actionable (toggle/edit inline) — same widget as Task List.
- Keep the big block-art number as a small accent, not the whole screen.
- Optional: a 7-day "week ahead" mini-column.

Decision to make: do we keep a separate Dashboard *and* Task List, or merge (Today = filtered
Task List)? Recommendation: **merge** — Today is Task List with the Smart grouping scrolled to
Today. Fewer views, less redundancy.

### 3.3 Statistics → Review
- Keep weekly completions bar chart + streak.
- Add: completion rate, avg age of open tasks, overdue trend, per-project breakdown.
- Completed list stays here as the archive/history with search.

### 3.4 Task edit sheet (new — collapses e/p/D/E)
A single modal/side-panel that edits **every** attribute of a task, opened with `Enter`/`e`:
```
┌ Edit Task ─────────────────────────┐
│ Name    [ Buy milk               ] │
│ Project [ @home                  ] │
│ Priority  ( ) Low (•) Med ( ) High │
│ Due     [ 2026-07-20 17:00       ] │  ← presets: t/T/r, empty = clear
│ Notes   [ multi-line …           ] │
│ [Tab] next field  [Ctrl+S] save  [Esc] cancel │
└────────────────────────────────────┘
```
`Tab`/`Shift+Tab` move between fields; reuse existing textarea for Notes. This one screen
removes four standalone keybindings and is the intuitive target of `Enter`.

### 3.5 Settings
Add: default view (Today vs List), default grouping, confirm-before-delete on/off,
date format, week-start day, 24h vs 12h clock. Keep the popup style.

---

## 4. Workflow features (make it best-in-class)

### 4.1 Natural-language quick-add (high impact)
Parse one capture line into structured fields (extend `parse_project`):
- `@project` → project (exists today)
- `!1`/`!2`/`!3` or `!high` → priority
- `^tomorrow`, `^fri`, `^2026-07-20`, `^"jul 20 5pm"` → due date/time
- `#tag` → optional labels (future)
Example: `Draft Q3 report @work !1 ^fri 5pm`. Show a live parse preview under the input.

### 4.2 Reschedule presets & scheduling
`t` = today, `T` = tomorrow, `w` = next week, `r` = open reschedule prompt. Overdue tasks get a
one-key "roll to today". This is the single biggest daily-driver feature in Todoist/Things.

### 4.3 Recurring tasks
`every day`/`every mon`/`every 2 weeks`. On completion, spawn the next occurrence. Needs a
`recurrence` field + generation logic. Medium effort, high value.

### 4.4 Safety: undo + confirm
- Soft-delete + `u` undo (keep a small ring of last actions) **or** a confirm prompt on `d`.
- Never lose data to a single keystroke.

### 4.5 Subtasks
Already specced in `.docs/task.md`. Fits naturally as expandable rows in the grouped list and a
progress badge `[2/5]`. Sequence after the grouping refactor (shared flattened-row model).

### 4.6 Nice-to-haves
- `?` help overlay (also serves as discoverability for the trimmed bar).
- Mouse support (ratatui supports click/scroll) — select + toggle.
- Bulk actions (visual/multi-select with `v`, then act on all).
- Import/export (JSON, or Todoist/Taskwarrior format).
- Colored priority left-border/gutter instead of a glyph for faster scanning.

---

## 5. Proposed roadmap (phased, each independently shippable)

**Phase 1 — Controls & safety (the explicit ask).** Trim the bar to the essential set (§2),
remap `Space`=done / `Enter`=edit / `a`=add, add `?` help overlay, add delete confirm-or-undo.
Touch: `src/main.rs` (routing), `src/ui/task_list.rs` (bar + help overlay), `src/app/ui_state.rs`.

**Phase 2 — Task edit sheet (§3.4).** One modal to edit all attributes; retire `e`/`p`/`Shift+D`/
`Shift+E` as standalone keys. Touch: new `src/ui/edit_sheet.rs`, `ui_state`, `main`.

**Phase 3 — Grouped agenda + Today home (§3.1, §3.2). ✅ DONE.** Section grouping cycled with
`g` (Smart / Project / Priority / Manual); Task List renders section headers with counts and
walks the flattened section order for both display and cursor navigation. Dashboard is merged
into the Task List as a compact top-of-view "Today" strip (date + overdue/today/done-today/open),
and `Tab` now toggles Task List ↔ Statistics only. `sort_mode` field replaced by `grouping_mode`;
DB reads the legacy key for lossless upgrades. Touched: `src/app/mod.rs` (grouping model),
`src/ui/task_list.rs` (headers + strip), `src/main.rs` (routing + `g` key), `src/db.rs`
(migration), `src/ui/dashboard.rs` (removed).

**Phase 4 — Quick-add parsing + reschedule presets (§4.1, §4.2). ✅ DONE.** New-task input parses
`@project`, `!priority` (`1`/`l`/`low` … `3`/`h`/`high`), and `^date` tokens via
`parse_quick_add`; the input block's title turns into a live parse preview so users see how
their capture line is interpreted. Reschedule presets `t` (today), `T` (tomorrow), `w` (next
week) act on the highlighted task; `r` opens a one-line prompt that accepts the same
shortcut vocabulary (`today`, `tomorrow`, `mon..sun`, `nw`/`next-week`, `YYYY-MM-DD`,
`YYYY-MM-DD HH:MM`; empty = clear). Rescheduling preserves the task's original time-of-day
when it wasn't a placeholder. Touched: `src/app/ui_state.rs` (parsers +
reschedule helpers), `src/app/mod.rs` (`InputMode::Rescheduling`, `App::set_active_due`),
`src/main.rs` (`t`/`T`/`w`/`r` routing), `src/ui/task_list.rs` (preview + prompt).

**Phase 5 — Recurring tasks + subtasks (§4.3, §4.5).** Data-model + generation logic; subtasks
per `.docs/task.md`.

**Phase 6 — Polish.** Review stats, mouse, bulk actions, import/export, priority gutter.

Recommendation: do **Phase 1 first and on its own** — it directly fixes the overflowing,
unintuitive control bar and makes the app safe to use daily, without waiting on the larger
structural work.
