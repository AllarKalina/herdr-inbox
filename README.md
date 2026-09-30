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

`start` remains available for creating a local record and Markdown file without launching an agent. `finish` marks the spec done and makes Jira linking available. A linked Jira ticket unlocks implementation; starting implementation unlocks draft PR linking. CLI and TUI enforce the same sequence. Existing active records retain their recorded progress. `list --json` and `show <id> --json` give agents structured state. The TUI refreshes from disk every second, so CLI updates appear there without a server.

The borderless inbox list uses the popup's translucent background without a darker inset. One cell of spacing separates the list and shortcuts from the popup edge. Each spec name is followed by fixed Spec, Jira, Dev, and PR status columns at the right edge. Long names truncate within their column instead of shifting the statuses. Below 64 columns, the list keeps the name and shows a compact `S J D P` icon trail; open a row for full status labels. Icons and words mark progress at normal widths: `○ locked`, `○ wait`, `→ ready`, `● active`, `✓ done`, and `◐ draft`; color is a secondary cue.

Hover or use `j/k` to select a row, then press Enter to open its detail view. The detail view shows the spec preview on the left and an interactive progress rail on the right. SPEC, JIRA, DEV, and PR share one straight vertical spine, with circular nodes, explicit states, and the available session, ticket, branch, or PR reference. The next actionable milestone is selected when you open an item. Use `j/k` or Up/Down to select a milestone, or click it with the mouse. Locked stages stay selectable so you can see the missing prerequisite.

Milestones keep their positions as you navigate. Rails at least 25 rows tall give every stage a fixed six-row slot, with four reserved control rows directly beneath its context. Shorter rails place the complete overview beside a stationary control area labeled with the selected stage. Actions and confirmation hints wrap on narrow terminals; long input scrolls horizontally to keep the cursor visible. Selection, locked guidance, text entry, and confirmations never push other milestones around. Only the selected stage label has a teal background; state colors stay readable.

Use Tab, `h/l`, or Left/Right to cycle actions; Enter or a mouse click runs the selected local flow. SPEC offers reading, editing, and finishing. JIRA offers linking when ready, then opening or updating its linked ticket. DEV offers starting when ready, then updating implementation details while active or at draft PR. PR offers linking when ready, then reviewing or updating its draft. A locked milestone explains its prerequisite in its control area. Input and confirmation prompts replace the selected stage's buttons. When progress advances, selection follows the next recommended stage if you were on the previous recommended stage; manually selected history stays selected. Below a body width of 78 columns, the 36-column right rail moves below the spec preview. At 40 × 18 and larger, all four milestones stay visible; short terminals prioritize the progress controls.

Press `r` for the full scrollable Markdown spec; use `j/k` to scroll one line or `Shift+J/K` to scroll ten. Scrolling stops two rows after its last rendered line and adjusts to wrapping, file edits, and resizing. Press `e` to edit it, Esc to return to the previous view, or `q` to close the inbox. The detail view keeps the same two-step deletion confirmation with `d`, then Enter.

List actions: Enter opens the selected spec; `n` starts a new spec session; `d` opens the deletion confirmation. Hover, arrow keys, or `j/k` select a row; Esc closes the inbox. Editing and stage actions live in the detail view. The CLI still supports creating local records without launching an agent. Status messages appear above the three action hints.
