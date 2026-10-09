# Architecture

How the code is laid out, and where a change of each kind goes. `PRODUCT.md` says what the Inbox is for and `DESIGN.md` how it looks; this file is about the code.

## Layout

```text
src/
  main.rs            parse arguments, run the CLI, report an error
  cli.rs             command table: help and dispatch read the same entries
  cli/               args (parser), output (text for people), items, sessions, config
  settings.rs        Settings and SpecSource; folder membership by physical path
  store.rs           Store: paths, lock, atomic writes
  store/
    model.rs         Record, Launch, Jira, PullRequest, Location
    workflow.rs      every status and every rule for moving between them
    records.rs       create, read, list, update
    config.rs        read and change the settings file
    archive.rs       archive, restore, delete (macOS Trash)
    scan.rs          discovery, pruning, ScanReport
    sources.rs       locate a spec in a folder, relink, relocate a folder
  launch.rs          start and refine spec sessions; one preflight for both
  launch/            profile (clients), herdr (CLI client), prompts, session (the steps)
  ui.rs              App, Screen dispatch, refresh, the event loop
  ui/
    screens/         one module per screen: list, detail (+ rail), reader, settings (+ picker), archive, scan
    modal.rs         prompts and the client choice: state, keys, submission (+ modal/view.rs)
    milestone.rs     stages as the UI sees them: state, actions, guidance
    chrome.rs        header, content area, notice row, shortcut line
    notice.rs, scroll.rs, text.rs, theme.rs, tree.rs
tests/               end-to-end tests through the real binary
tests/snapshots/ui/  golden snapshots of every screen
scripts/regress      the regression gate
```

## Rules that hold everywhere

- **Files decide what is in the Inbox.** A record appears only when its spec file exists inside a selected folder (`ui::tree::includes`, `settings::source_location`). Metadata never adds a row.
- **One place per rule.** Workflow prerequisites live in `store/workflow.rs` and nowhere else; the UI and CLI ask it. Colours live in `ui/theme.rs`, text fitting in `ui/text.rs`.
- **Statuses are enums.** Their stored names are fixed by the `status!` macro. Match on variants; never compare strings outside tests.
- **Every write goes through the store**, under its lock, as an atomic replace. Settings change only through `Store::update_settings`.
- **Errors bubble up.** Screens and commands return `Result`. The event loop turns a failure into the red notice; the CLI prints it and exits 1. Nothing swallows an error or substitutes a default.
- **One schema, no migrations.** Settings and records are schema 1. A file from another version is rejected, never rewritten.
- **Rust files stay under 500 lines.** The gate enforces it; split by responsibility.

## Adding things

### A screen

1. Add `src/ui/screens/<name>.rs` with a `View` (its state) and `draw`, `handle_key`, `handle_mouse` and `crumbs`.
2. Register it in `src/ui/screens.rs`, add a field to `App` and a variant to `Screen` in `src/ui.rs`. The compiler then lists every `match` that needs an arm.
3. Draw inside `chrome::content`, put shortcuts in `chrome::draw_footer`, and report outcomes with `app.notice`.
4. Add a scenario to `src/ui/tests/golden/` and to the size sweep in `src/ui/tests/resize.rs`.

### A setting

1. Add the field to `Settings` (it is required in `settings.toml`; say so in the README).
2. Add a `Row` variant in `src/ui/screens/settings.rs` and fill in `label`, `verb`, `value` and `activate`.
3. If scripts need it, add an `Action` in `src/cli/config.rs`.

### A CLI command

Add one `Command::new(name, usage, function)` to the table in `src/cli/items.rs`, `sessions.rs` or `config.rs`. Take flags with `Args::flag`/`switch`, then call `args.positionals()` so stray arguments are refused. Help is generated from the table. Cover it in `tests/cli_workflow.rs` or `tests/cli_config.rs`.

### A workflow stage or step

1. Add the status and the rule in `src/store/workflow.rs` (`status!`, `Change`, `Record::apply`, the `*_stage` functions).
2. Add the `Milestone` variant in `src/ui/milestone.rs`; `state`, `actions`, `context` and `guidance` then need an arm each. The list columns and the progress rail follow `Milestone::visible`.
3. Add its prompts to `Prompt` in `src/ui/modal.rs` and its actions to `DetailAction`.

### An agent client

Add a variant to `Profile` in `src/launch/profile.rs` and fill in its arms: id, label, default model, executable, skill invocation, and its command-line arguments in `arguments`. Nothing else knows about individual clients.

### A prompt

Add a variant to `Prompt` in `src/ui/modal.rs` with its `label`, say in `item` which item it acts on, and handle it in `submit`. Open it with `app.begin` or `app.begin_with`.

## Tests

- **Golden snapshots** (`src/ui/tests/golden/`, files in `tests/snapshots/ui/`) pin how every screen looks: text and styling at three sizes. Do not assert on rendered text anywhere else.
- **Behaviour tests** (`src/ui/tests/*.rs`) pin what keys and clicks do to the state and the store. They send input through `dispatch`, the same entry point as the event loop.
- **Workflow rules** are tested as pure functions in `src/store/tests/workflow.rs`; the rest of `src/store/tests/` covers scanning, folders and the archive on disk.
- **End-to-end** tests in `tests/` drive the real binary: `cli_workflow.rs` and `cli_config.rs` for the CLI contract, `launch_flow.rs` and `refine_flow.rs` for sessions against a fake `herdr`.
- `scripts/regress` runs all of it, plus lint, policy checks, the release build and two smoke runs. Run it after every change.

Test one behaviour at one layer: rules in the store, contracts through the CLI, input handling in the UI, appearance in snapshots.
