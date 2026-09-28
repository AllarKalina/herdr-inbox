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

The action opens an 85%-size Herdr popup. You can also run `~/git/herdr-inbox/target/release/herdr-inbox tui` in any terminal. No keybinding is claimed by default.

## Workflow

```sh
~/git/herdr-inbox/target/release/herdr-inbox start "Payment retry handling" --repo ~/git/my-service
~/git/herdr-inbox/target/release/herdr-inbox show <id>
~/git/herdr-inbox/target/release/herdr-inbox finish <id>
~/git/herdr-inbox/target/release/herdr-inbox jira <id> ABC-123 --url https://jira.example/browse/ABC-123
~/git/herdr-inbox/target/release/herdr-inbox implement <id> --agent implementor --branch feature/payment-retries
~/git/herdr-inbox/target/release/herdr-inbox pr <id> https://github.example/org/repo/pull/123
```

`start` creates the record and a Markdown file, then marks the spec in progress. `finish` marks the spec done and makes both Jira creation and implementation handoff available. Jira and implementation can proceed independently. `list --json` and `show <id> --json` give agents structured state. The TUI refreshes from disk every second, so CLI updates appear there without a server.

TUI keys: `n` start a spec; `e` open its Markdown file in `$EDITOR` (default `code`); `f` finish; `J` link Jira; `i` start implementation; `p` link draft PR; `j/k` navigate; `q` close.
