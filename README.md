# Personal inbox

A local Herdr inbox for moving an idea through spec, Jira, implementation, and draft PR. This is a personal plugin; it is linked from this directory and has no publishing step.

## Storage

Each item is a JSON file in `~/Library/Application Support/herdr-inbox/items/`. New specs are Markdown files in `~/Library/Application Support/herdr-inbox/specs/` unless `--spec` points elsewhere. `HERDR_INBOX_HOME` overrides the data directory for backup or testing. The plugin has no hosted database, sync service, or background daemon. Jira and PR URLs are references to external services; linking them does not create anything remotely.

## Install locally

```sh
cargo build --release
herdr plugin link ~/git/herdr-inbox
herdr plugin action invoke personal.inbox.open
```

The action opens an 85%-size Herdr popup. `Cmd+I` opens it with the personal Ghostty/Herdr config. You can also run `~/git/herdr-inbox/target/release/herdr-inbox tui` in any terminal.

## New spec session

Press `n` in the inbox. It asks for the target workspace (default `AI herd`), optional repo directory, Claude model (default `claude-opus-5-5`), and optional topic. It creates an untitled inbox item, opens a temporary `Spec · <id>` tab in that workspace, starts Claude Code at high effort with bypass permissions, and submits `/grill-me` as the first prompt. The prompt tells Claude where to write the Markdown spec and how to give the item its final title when the grilling session ends. The inbox popup closes on successful launch so the new tab is visible.

The target Herdr workspace, Claude Code executable, and `/grill-me` skill must already exist in the environment where you run this. A missing workspace or Claude executable leaves the inbox unchanged. A failure after tab creation leaves the item and tab in place with an error recorded for inspection; the plugin never silently closes a working session.

CLI equivalent:

```sh
~/git/herdr-inbox/target/release/herdr-inbox launch --workspace "AI herd" --repo ~/git/my-service --model claude-opus-5-5 --topic "Improve payment retries"
```

Use `--effort` to change effort or `--ask-permissions` to disable the bypass default for one launch. The title is supplied after the spec is written:

```sh
~/git/herdr-inbox/target/release/herdr-inbox finish <id> --title "Payment retry handling"
```

## Workflow

```sh
~/git/herdr-inbox/target/release/herdr-inbox start "Payment retry handling" --repo ~/git/my-service
~/git/herdr-inbox/target/release/herdr-inbox show <id>
~/git/herdr-inbox/target/release/herdr-inbox finish <id>
~/git/herdr-inbox/target/release/herdr-inbox jira <id> ABC-123 --url https://jira.example/browse/ABC-123
~/git/herdr-inbox/target/release/herdr-inbox implement <id> --agent implementor --branch feature/payment-retries
~/git/herdr-inbox/target/release/herdr-inbox pr <id> https://github.example/org/repo/pull/123
```

`start` remains available for creating a local record and Markdown file without launching an agent. `finish` marks the spec done and makes both Jira creation and implementation handoff available. Jira and implementation can proceed independently. `list --json` and `show <id> --json` give agents structured state. The TUI refreshes from disk every second, so CLI updates appear there without a server.

TUI keys: `n` launch a new spec session; `a` add a local record; `e` open its Markdown file in `$EDITOR` (default `code`); `f` finish and name an untitled spec; `t` rename; `J` link Jira; `i` start implementation; `p` link draft PR; `j/k` navigate; `q` close.
