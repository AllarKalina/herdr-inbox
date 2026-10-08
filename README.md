# Personal inbox

A local Herdr inbox for moving an idea through spec, Jira, implementation, and draft PR. Connect the same plugin to entirely different specs and context on each Mac. Content stays in user-selected folders; settings and workflow metadata stay local to that computer.

## Storage

Settings live in `~/Library/Application Support/herdr-inbox/settings.toml`; each item is a JSON file in `items/` under that directory. `HERDR_INBOX_HOME` overrides the entire local data root, including settings and Trash. No hosted database, sync service, or background daemon is involved. Plugin builds and upgrades leave this data untouched.

Press `s` for **Settings**. Add any spec folder; nested `.md` and `.markdown` files are discovered automatically when settings are saved and when the Inbox opens. `S` rescans while open. There is no required directory tree or sidecar file. Include/exclude rules use glob patterns relative to each source root; turn recursion off to scan only its immediate files. Symlink aliases and overlapping roots are deduplicated; directory cycles are skipped. Source content is never edited by discovery.

Imported specs start **Spec done**, Jira ready, Dev/PR locked. Titles come from the first Markdown H1, falling back to the filename. Repeated scans preserve UUIDs, titles, progress, timestamps, links, and launch history. Missing sources or specs retain their records and produce feedback. Context references can be files or folders; their paths are supplied to new/refinement sessions and checked before launch.

Settings also provide the preferred client and workspace. New sessions use an editable exact spec destination; leaving it blank generates a filename directly in the first configured source. Without sources, legacy creation still uses the local `specs/` directory. Refinement keeps its exact existing file and UUID.

Deletion (`d`, then Enter) moves metadata to local `trash/items/`. Explicitly Inbox-owned legacy specs move to `trash/specs/`; user-owned specs stay in place. Deleted references remain suppressed across scans. Press `u` in the list or choose Restore in settings to restore from Trash, or run `restore ID`; restoration keeps the original UUID and enriched progress.

Source IDs are local identities. Add an unrelated folder as a new source. Use the confirmed relocation flow only when moving the same source; matching unchanged files retain item identities. Changed or ambiguous files require explicit `relink ID PATH` (also `L` in detail). Unresolved spec sessions block relocation/relinking: finish the spec normally, or after stopping an abandoned session run `settle ID` (or `x`, then Enter, in detail). Settling records completion of the local session without advancing the spec workflow or controlling the agent. Settings edits apply to future launches; existing prompts retain their item, file and data-root targets.

CLI setup example:

```sh
herdr-inbox settings add-source "$HOME/my-specs"
herdr-inbox settings add-context "$HOME/my-project-context"
herdr-inbox settings defaults --profile codex --workspace ai-boiler-room
herdr-inbox settings show --json
herdr-inbox scan --json
```

Other settings commands: `add-source PATH --flat --include '**/*.md' --exclude 'archive/**'`, `remove-source ID`, `remove-context PATH`, and `relocate-source ID PATH --confirm`. Removing a source only changes discovery configuration; existing records and content remain.

Metadata JSON schema 1 includes `id`, `spec_path`, `source_id`, `source_relative_path`, `ownership` (`user` or `managed`), title/timestamps, `spec`, `jira`, `implementation`, `pr`, `launch`, and `previous_launches`. `content_fingerprint` supports confirmed relocation; it never identifies items by content alone. Legacy records keep their IDs and paths and receive explicit ownership on migration, without moving their content. Migration is additive; the original JSON remains unchanged until a validated write. Unsupported future settings/metadata schemas are rejected without rewriting them. Older plugin binaries must not write records after this upgrade.

For a separate Jira/Git enrichment script, enumerate with `list --json`, correlate `spec_path`, and update the UUID using `jira`, then `implement`, then `pr`. Inspect with `show ID --json`. Those commands share the UI's validation, locking, and atomic writes; rescanning preserves their updates. Import never infers links or creates remote work.

## Install on another Mac

Requires macOS, Herdr **0.9.1 or newer**, Git, Rust **1.89 or newer** (`cargo` on `PATH`), and Xcode Command Line Tools for the native linker. Installation builds for the Mac's own architecture, so the same command works on Apple Silicon and Intel:

```sh
herdr plugin install AllarKalina/herdr-inbox --ref main
herdr plugin list --plugin personal.inbox
```

Herdr runs the manifest's locked release build and registers the plugin locally, even before a server is running. It does not install Rust or other prerequisites. Start or attach to Herdr normally, then press **Cmd+I** with the shared Ghostty/Herdr config. Config sync carries the shortcut, but each Mac must install the plugin separately; do not copy Herdr's plugin registry or another Mac's compiled binary. See [Herdr's install and build reference](https://github.com/herdrdev/herdr/blob/master/docs/versions/0.9.1/website/src/content/docs/plugins.mdx).

For the shared setup, the Herdr config needs:

```toml
[[keys.command]]
key = "cmd+i"
type = "plugin_action"
command = "personal.inbox.open"
description = "Open personal inbox"
```

Ghostty must also pass the shortcut through:

```ini
keybind = super+i=unbind
```

Reload the configs after restoring them. Install the configured **JetBrainsMono Nerd Font Mono** on each Mac. For spec sessions, also install and sign in to the chosen Claude or Codex client, make your real `grill-me` skill discoverable to that client, and have a Herdr workspace named `ai-boiler-room`. Inspect existing workspaces with `herdr workspace list` before creating one. Skills belong in the main AI configuration, outside this plugin repository.

Inbox items, specs, Trash, client credentials, and live tabs are not distributed with this repository. New installs use `~/Library/Application Support/herdr-inbox/` on that computer; leave `HERDR_INBOX_HOME` unset for separate local inboxes.

The local content-source feature is available on `main`; use a version tag once this version is released.

To update a managed install, close the inbox and repeat `herdr plugin install AllarKalina/herdr-inbox --ref <new-tag>`. Herdr replaces its managed plugin checkout; inbox data stays in the separate local data directory. A locally linked development copy must be unlinked before switching that Mac to a managed install; development can keep using the link below.

## Develop locally

```sh
cargo build --release --locked
herdr plugin link ~/git/herdr-inbox
herdr plugin action invoke personal.inbox.open
```

The action opens an 85%-size Herdr popup. `Cmd+I` opens it with the personal Ghostty/Herdr config. You can also run `~/git/herdr-inbox/target/release/herdr-inbox tui` in any terminal.

## New spec session

Press `n` in the inbox, then choose an installed client: **Claude · Opus 5.5 · High** or **Codex · GPT-6.1-Sol · High**. The picker lists only clients found on `PATH`. Enter the target workspace (default `ai-boiler-room`), optional repo directory, exact spec destination, and optional topic. The inbox creates an untitled item and a `Spec · <id>` tab, starts the selected agent, and sends `/grill-me` to Claude or `$grill-me` to Codex. The prompt gives the agent the Markdown spec path and the command to set the final title after the session. The inbox popup closes on successful launch so the tab is visible.

The target Herdr workspace, selected client, and its `grill-me` skill must exist where you run this. Claude defaults to Opus 5.5 at High effort with bypass permissions. Codex defaults to GPT-6.1-Sol (`gpt-6.1-sol`) at High effort with `workspace-write` sandboxing and added writable Inbox and spec-parent directories. A missing context reference, workspace or client leaves the inbox unchanged. A failure after tab creation leaves the item and tab in place with an error recorded for inspection.

CLI equivalent:

```sh
~/git/herdr-inbox/target/release/herdr-inbox profiles
~/git/herdr-inbox/target/release/herdr-inbox launch --profile codex --workspace ai-boiler-room --repo ~/git/my-service --topic "Improve payment retries"
```

`--profile opus` selects Claude. `--model` and `--effort` override a profile's defaults; `--ask-permissions` disables Claude's bypass default for one launch. The title is supplied after the spec is written:

```sh
~/git/herdr-inbox/target/release/herdr-inbox finish <id> --title "Payment retry handling"
```

In the personal setup, the real skill lives at `~/git/codex/skills/grill-me/SKILL.md`, linked into `~/.codex/skills/grill-me`. Restore the skill through the main AI configuration on each Mac. The plugin supplies the skill invocation and inbox context; it does not bundle or install a skill.

## Refine an existing spec

Open an item, select SPEC, and choose **Refine the spec**. The installed-client chooser opens again; choosing Claude or Codex creates a new grilling tab in `ai-boiler-room` for the same inbox item. Its title, repo, and Markdown spec path are retained. Cancelling the chooser changes nothing. The client defaults and permission modes match a new session.

The agent first reads the existing spec and item context, uses the services, files, and paths named in the spec to locate the affected codebases, and validates its claims against current code. Then it reports discrepancies or missing context, asks what you want to change, challenges assumptions, and continues the interview. Code is the source of truth. The existing Markdown file is updated only after the required decisions are made, and the agent finishes the same item ID. Refinement does not create another inbox item or erase Jira, implementation, or PR links and progress.

Once the harness accepts the prompt, SPEC becomes active. That means the session has started, not that the agent has completed its code validation. **Seal the spec** or the agent's `finish` command marks it done again while retaining downstream progress. A launch failure preserves the prior spec status and never overwrites the Markdown file; its error remains available for inspection. If workspace focusing fails after the prompt was accepted, the session stays active and its warning asks you to select the new tab manually. Pressing `e` in the detail view or full reader still opens the raw editor; **Refine the spec** launches the AI interview.

CLI equivalent, run inside Herdr after setting `INBOX_ITEM_ID` to an existing item's ID:

```sh
"$HOME/git/herdr-inbox/target/release/herdr-inbox" refine "$INBOX_ITEM_ID" --profile codex
"$HOME/git/herdr-inbox/target/release/herdr-inbox" refine "$INBOX_ITEM_ID" --profile opus --repo "$HOME/git/my-service"
```

`--repo` overrides the stored repo for that launch; omit it to reuse the item's repo. `refine` accepts the same profile, workspace, model, effort, topic, and permission overrides as `launch`; `--spec` is only for a new launch. Ensure the real `grill-me` skill from your main AI configuration is discoverable by the chosen client; the inbox passes the skill invocation rather than installing it.

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

Hover or use `j/k` to select a row, then press Enter to open its detail view. The detail view shows the spec preview on the left and an interactive progress rail on the right. SPEC, JIRA, DEV, and PR share one straight vertical spine, with circular nodes, explicit states, and available ticket, branch, or PR references. SPEC has no additional session metadata. The next actionable milestone is selected when you open an item. Use `j/k` or Up/Down to select a milestone, or click its five-by-three node area. A teal ring surrounds the selected node; its center keeps the status color. Locked stages stay selectable so you can see the missing prerequisite.

Milestones keep their positions as you navigate. Every node reserves five columns by three rows for its ring, selected or idle. Rails at least 26 rows tall give every stage a fixed six-row slot with reserved control space. Shorter rails place the complete 18-column overview beside a stationary control area labeled with the selected stage. Actions and confirmation hints wrap on narrow terminals; long input scrolls horizontally to keep the cursor visible. Selection, locked guidance, text entry, and confirmations never push other milestones around. Stage labels and controls have no background fill; the selected action uses `✦` and bold, underlined teal text.

Completing an action leaves a short green acknowledgement beside its milestone, rather than in the footer. It occupies the existing context row, so the layout stays steady; compact terminals use a shorter caption. Automatic selection of the next step keeps the acknowledgement attached to the completed stage. Any manual milestone navigation or leaving the detail view dismisses it, including navigating back to the stage you just completed.

Use Tab, `h/l`, or Left/Right to cycle actions; Enter or a mouse click runs the selected flow. SPEC offers **Seal the spec**, **Read the scroll**, and **Refine the spec**; refinement opens the client chooser and starts a new AI interview for the existing spec. JIRA offers **Bind Jira ticket**, then **Visit Jira ticket** and **Update Jira link**. DEV offers **Log dev quest**, then **Update dev quest**; both record agent and branch information rather than launching an implementer. PR offers **Bind draft PR**, then **Review draft PR** and **Update PR link**. A locked milestone explains its prerequisite in its control area. Input and confirmation prompts replace the selected stage's controls. When progress advances, selection follows the next recommended stage if you were on the previous recommended stage; manually selected history stays selected. Below a body width of 78 columns, the 36-column right rail moves below the spec preview. At 40 × 18 and larger, all four milestones stay visible; short terminals prioritize the progress controls.

Press `r` for the full scrollable Markdown spec; use `j/k` to scroll one line or `Shift+J/K` to scroll ten. Scrolling stops two rows after its last rendered line and adjusts to wrapping, file edits, and resizing. Press `e` to edit it, Esc to return to the previous view, or `q` to close the inbox. The detail view keeps the same two-step deletion confirmation with `d`, then Enter.

List actions: `s` settings; `S` rescan; `u` Trash; Enter opens the selected spec; `n` starts a new spec session; `d` opens the deletion confirmation. Hover, arrow keys, or `j/k` select a row; Esc closes the inbox. Editing and stage actions live in the detail view. The CLI still supports creating local records without launching an agent. Status messages appear above the three action hints.
