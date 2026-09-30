# Personal inbox

A local Herdr inbox for moving an idea through spec, Jira, implementation, and draft PR. This is a personal plugin; it is linked from this directory and has no publishing step.

## Storage

Each item is a JSON file in `~/Library/Application Support/herdr-inbox/items/`. New specs are Markdown files in `~/Library/Application Support/herdr-inbox/specs/` unless `--spec` points elsewhere. `HERDR_INBOX_HOME` overrides the data directory for backup or testing. The plugin has no hosted database, sync service, or background daemon. Jira and PR URLs are references to external services; linking them does not create anything remotely.

To remove an item, select it and press `d`. The confirmation shows its title, ID, spec path, and which files will move. Press Enter to delete or Esc to cancel. Deleted records move to `trash/items/` under the inbox data directory; inbox-owned specs move to `trash/specs/`. Specs linked from elsewhere stay in place. Open agent tabs and external Jira/PRs are unaffected. The files remain on disk for manual recovery.

## Install locally

```sh
cargo build --release
herdr plugin link ~/git/herdr-inbox
herdr plugin action invoke personal.inbox.open
```

The action opens an 85%-size Herdr popup. `Cmd+I` opens it with the personal Ghostty/Herdr config. You can also run `~/git/herdr-inbox/target/release/herdr-inbox tui` in any terminal.

## New spec session

Press `n` in the inbox, then choose an installed client: **Claude · Opus 5.5 · High** or **Codex · GPT-6-Sol · High**. The picker lists only clients found on `PATH`. Enter the target workspace (default `ai-boiler-room`), optional repo directory, and optional topic. The inbox creates an untitled item and a `Spec · <id>` tab, starts the selected agent, and sends `/grill-me` to Claude or `$grill-me` to Codex. The prompt gives the agent the Markdown spec path and the command to set the final title after the session. The inbox popup closes on successful launch so the tab is visible.

The target Herdr workspace, selected client, and its `grill-me` skill must exist where you run this. Claude starts with bypass permissions; Codex receives write access to the local inbox data directory. A missing workspace or client leaves the inbox unchanged. A failure after tab creation leaves the item and tab in place with an error recorded for inspection.

CLI equivalent:

```sh
~/git/herdr-inbox/target/release/herdr-inbox profiles
~/git/herdr-inbox/target/release/herdr-inbox launch --profile codex --workspace ai-boiler-room --repo ~/git/my-service --topic "Improve payment retries"
```

`--profile opus` selects Claude. `--model` and `--effort` override a profile's defaults; `--ask-permissions` disables Claude's bypass default for one launch. The title is supplied after the spec is written:

```sh
~/git/herdr-inbox/target/release/herdr-inbox finish <id> --title "Payment retry handling"
```

For a local Codex trial, use `--repo ~/git/herdr-inbox/examples/mock-repo`. Its [mock grill-me skill](examples/mock-repo/.agents/skills/grill-me/SKILL.md) asks two scoping questions and writes a short test spec. It does not create Jira tickets or PRs. Use your real skill in work repos.

## Workflow

```sh
~/git/herdr-inbox/target/release/herdr-inbox start "Payment retry handling" --repo ~/git/my-service
~/git/herdr-inbox/target/release/herdr-inbox show <id>
~/git/herdr-inbox/target/release/herdr-inbox finish <id>
~/git/herdr-inbox/target/release/herdr-inbox jira <id> ABC-123 --url https://jira.example/browse/ABC-123
~/git/herdr-inbox/target/release/herdr-inbox implement <id> --agent implementor --branch feature/payment-retries
~/git/herdr-inbox/target/release/herdr-inbox pr <id> https://github.example/org/repo/pull/123
~/git/herdr-inbox/target/release/herdr-inbox delete <id> --confirm <full-id>
```

`start` remains available for creating a local record and Markdown file without launching an agent. `finish` marks the spec done and makes both Jira creation and implementation handoff available. Jira and implementation can proceed independently. `list --json` and `show <id> --json` give agents structured state. The TUI refreshes from disk every second, so CLI updates appear there without a server.

The borderless inbox list uses the popup's translucent background without a darker inset. One cell of spacing separates the list and shortcuts from the popup edge. Each spec name is followed by fixed Spec, Jira, Dev, and PR status columns at the right edge. Long names truncate within their column instead of shifting the statuses. Icons and words mark progress: `○ wait`, `→ ready`, `● active`, `✓ done`, and `◐ draft`; color is a secondary cue.

Hover or use `j/k` to select a row, then press Enter to open its detail view. The detail view gives the spec preview room to breathe, then shows a quest path with Jira and implementation as parallel branches. Tab or arrow keys select a next-action button; Enter or a mouse click runs the existing local flow. Jira linking asks for a key and an optional URL; once a draft PR is linked, its action opens the PR for review. Press `r` for the full scrollable Markdown spec; use `j/k` to scroll one line or `Shift+J/K` to scroll ten. Scrolling stops two rows after its last rendered line and adjusts to wrapping, file edits, and resizing. Press `e` to edit it, Esc to return to the previous view, or `q` to close the inbox. The detail view keeps the same two-step deletion confirmation with `d`, then Enter.

List actions: Enter opens the selected spec; `n` starts a new spec session; `d` opens the deletion confirmation. Hover, arrow keys, or `j/k` select a row; Esc closes the inbox. Editing and stage actions live in the detail view. The CLI still supports creating local records without launching an agent. Status messages appear above the three action hints.
