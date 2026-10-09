# Personal inbox

A local Herdr inbox for moving an idea through spec, Jira, implementation, and draft PR. Connect the same plugin to entirely different specs and context on each Mac. Content stays in user-selected folders; settings and workflow metadata stay local to that computer.

## Storage

Settings live in `~/Library/Application Support/herdr-inbox/settings.toml`; each item is a JSON file in `items/` under that directory. `HERDR_INBOX_HOME` overrides the entire local data root, including settings and archived items. No hosted database, sync service, or background daemon is involved. Plugin builds and upgrades leave this data untouched.

Press `s` for **Settings**. The page lists three rows with their current values: **Specs folder**, **Jira**, and **Archive**. Use `j/k` or hover to select a row and Enter or a click to act on it. On **Specs folder**, Enter opens the native macOS folder selector. Choosing a folder immediately persists the selection and scans its nested `.md` and `.markdown` files; cancelling leaves the current selection and Inbox unchanged. There is no save step, manual path field, or discovery-options menu. With no folder selected, the same row connects one. There is no required directory tree or sidecar file. Symlink aliases and overlapping roots are deduplicated; directory cycles are skipped. Source content is never edited by discovery.

Imported specs start **Spec done**, Jira ready, Dev/PR locked. Titles come from the first Markdown H1, falling back to the filename. Repeated scans preserve UUIDs, titles, progress, timestamps, links, and launch history. The Inbox shows only existing files inside currently selected source folders that match their discovery filters. Actual filesystem paths determine membership; cached source IDs and relative paths do not authorize visibility. Missing files, removed sources, and metadata for other folders never appear in the Inbox. Each scan also drops the record of a spec whose file was deleted from its selected folder, along with its progress and links; only the metadata is removed. A folder that cannot be read drops nothing, and a new spec whose session has not written its file yet is kept. A file renamed inside the folder therefore returns as a fresh item. Context references can be files or folders; their paths are supplied to new/refinement sessions and checked before launch.

The CLI configures context references, the preferred client, and the development skill. New sessions require a selected source folder; the session itself chooses where inside it the spec goes. With no selected folders, the Inbox opens Settings and stays empty. Refinement keeps its exact existing file and UUID.

Archiving (`a`, then Enter) moves metadata to the local `archive/` folder; user-selected spec files stay in place. Archived references remain suppressed across scans. The archive holds only specs that still exist inside the selected folders: a scan drops an archived record once its file is gone, and selecting another folder drops the archived records of the old one. Their files are never touched, so those specs appear as fresh items if that folder is selected again. The CLI's `archive` command performs the same recoverable operation.

The **Archive** row in Settings opens the archive list at `Inbox / Settings / Archive`. Press `r` to restore the selected spec with its original UUID and enriched progress (CLI: `restore ID`). Press `d`, then Enter, to delete it: the archived record is removed and the spec file moves to the macOS Trash, so it no longer returns on the next scan. Esc cancels the confirmation. Delete is the only Inbox action that touches a spec file; recover it from the Trash in Finder.

The **Jira** row turns the Jira stage on or off for this computer (`jira = true` or `false` in `settings.toml`, required). Off, the workflow is Spec → Dev → PR: the Jira column and milestone are hidden, implementation needs only a finished spec, and the CLI `jira` command is refused. Stored Jira links are kept and reappear when Jira is turned back on; a spec that progressed without a ticket then shows Jira as its outstanding step.

Source IDs are local identities. Settings' **Change** selects the Inbox folder. The CLI also supports adding sources and confirmed relocation when moving the same source; matching unchanged files retain item identities. Changed or ambiguous files require explicit `relink ID PATH` through the CLI. If a scan already imported the moved file as a fresh, untouched item, relinking replaces that placeholder; an item with any progress, or an archived one, blocks the relink instead. Unresolved spec sessions block relocation/relinking: finish the spec normally, or after stopping an abandoned session run `settle ID` (or `x`, then Enter, in detail). Settling records completion of the local session without advancing the spec workflow or controlling the agent. Settings edits apply to future launches; existing prompts retain their item, file and data-root targets.

CLI setup example:

```sh
herdr-inbox settings add-source "$HOME/my-specs"
herdr-inbox settings add-context "$HOME/my-project-context"
herdr-inbox settings defaults --profile codex
herdr-inbox settings show --json
herdr-inbox scan --json
```

Additional CLI settings commands: `add-source PATH --flat --include '**/*.md' --exclude 'archive/**'`, `remove-source ID`, `remove-context PATH`, and `relocate-source ID PATH --confirm`. These options are not exposed in the Settings page. Removing a source immediately removes its items from the Inbox and empties the archive of its specs. Files, and the records of its unarchived items, remain on disk; neither can override the current source selection.

Metadata JSON schema 1 includes `id`, `spec_path`, `source_id`, `source_relative_path`, title/timestamps, `spec`, `jira`, `implementation`, `pr`, `launch`, and `previous_launches`. Source files always belong to the user; there is no managed-file ownership mode. `content_fingerprint` supports confirmed relocation; it never identifies items by content alone. Only the current settings and metadata schemas are supported. Development removes obsolete formats and paths rather than maintaining migrations, compatibility fallbacks, or synthetic Inbox content. CLI metadata inspection remains separate from the Inbox's filesystem membership.

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

Inbox items, specs, archives, client credentials, and live tabs are not distributed with this repository. New installs use `~/Library/Application Support/herdr-inbox/` on that computer; leave `HERDR_INBOX_HOME` unset for separate local inboxes.

The local content-source feature is available on `main`; use a version tag once this version is released.

To update a managed install, close the inbox and repeat `herdr plugin install AllarKalina/herdr-inbox --ref <new-tag>`. Herdr replaces its managed plugin checkout; inbox data stays in the separate local data directory. A locally linked development copy must be unlinked before switching that Mac to a managed install; development can keep using the link below.

## Develop locally

```sh
cargo build --release --locked
herdr plugin link ~/git/herdr-inbox
herdr plugin action invoke personal.inbox.open
```

### Regression gate

Run the gate after every change:

```sh
scripts/regress                     # everything, under a minute
scripts/regress --quick             # skip the release build and smoke runs
scripts/regress --update-snapshots  # re-record UI snapshots after an intended UI change
```

It runs formatting, clippy with warnings denied, all tests, repository policy checks, the manifest's release build, a CLI smoke run against the release binary, and a live TUI smoke run in a pseudo-terminal. Every stage uses a temporary data directory, never your Inbox. Logs land in `target/regress/`; the exit status is 0 only when every stage passes. CI runs the same script on macOS.

`tests/snapshots/ui/` holds a golden snapshot of every screen at 100×35, 60×24, and 40×18: the rendered text, then the colour and weight of each styled run. A layout, copy, or colour change fails the gate and names the first differing line; the full actual output is written to `target/ui-snapshots-actual/`. Re-record only when the change is intended, then review `git diff tests/snapshots`. `tests/cli_workflow.rs` and `tests/cli_config.rs` drive the CLI through the real binary, including the whole spec-to-PR path with Jira on and off. [ARCHITECTURE.md](ARCHITECTURE.md) maps the code and lists the steps for adding a screen, setting, command, workflow step, or client. Launching real agent sessions is covered against a fake `herdr` binary only.

The action opens an 85%-size Herdr popup. Its frame has no visible caption; the `Inbox` breadcrumb belongs inside the application. Herdr requires a nonempty pane title, so the manifest uses a nonprinting zero-width space. After editing a locally linked manifest, rerun `herdr plugin link ~/git/herdr-inbox` and reopen the popup. `Cmd+I` opens it with the personal Ghostty/Herdr config. You can also run `~/git/herdr-inbox/target/release/herdr-inbox tui` in any terminal.

## New spec session

Press `n` in the inbox. One **New spec** panel asks for everything: the client, **Claude · Opus 5.5 · Medium** or **Codex · GPT-6.1-Sol · Medium** (only clients found on `PATH` are listed, starting on your preferred one), and an optional topic. Up/Down or Tab pick the client, typing fills in the topic, Enter starts the session, Esc cancels. A long topic scrolls so the cursor stays visible, and a failed start explains itself above the shortcuts with the panel still open to retry.

Starting creates an untitled item and a `Spec · <id>` tab in the `ai-boiler-room` workspace, starts the selected agent, and sends `/grill-me` to Claude or `$grill-me` to Codex. Sessions always run in that workspace: it is where agents are set up, and it is not a setting. The popup closes on success so the tab is visible.

The session decides where the spec goes. The prompt names your specs folder, not a file: the agent picks the existing subfolder that fits the subject, names the file, names the affected repositories in the spec, and reports the file when it finishes:

```sh
herdr-inbox finish <id> --title "Payment retry handling" --spec ~/my-specs/payments/retry-handling.md
```

Until then the item has no file and stays out of the Inbox. If the Inbox was opened in between and already listed the new file, finishing replaces that untouched entry with the session's item.

The selected client and its `grill-me` skill must exist where you run this, along with a Herdr workspace named `ai-boiler-room`. Claude runs Opus 5.5 at Medium effort in `auto` permission mode. Codex runs GPT-6.1-Sol (`gpt-6.1-sol`) at Medium effort with `workspace-write` sandboxing and added writable Inbox and specs directories. A missing context reference, workspace or client leaves the inbox unchanged. A failure before a tab exists also leaves the inbox unchanged. A failure after tab creation leaves the item and tab in place with an error recorded for inspection.

CLI equivalent:

```sh
~/git/herdr-inbox/target/release/herdr-inbox profiles
~/git/herdr-inbox/target/release/herdr-inbox launch --profile codex --topic "Improve payment retries"
```

`--profile opus` selects Claude. `--model` and `--effort` override a profile's defaults; `--ask-permissions` starts Claude without a permission mode for one launch, so it asks as usual.

In the personal setup, the real skill lives at `~/git/ai-boiler-room/skills/grill-me/SKILL.md`, linked into `~/.codex/skills/grill-me`. Restore the skill through the main AI configuration on each Mac. The plugin supplies the skill invocation and inbox context; it does not bundle or install a skill.

## Refine an existing spec

Open an item, select SPEC, and choose **Refine the spec**. The installed-client chooser opens again; choosing Claude or Codex creates a new grilling tab in `ai-boiler-room` for the same inbox item. Its title and Markdown spec path are retained. Cancelling the chooser changes nothing. The client defaults and permission modes match a new session.

The agent first reads the existing spec and item context, uses the services, files, and paths named in the spec to locate the affected codebases, and validates its claims against current code. Then it reports discrepancies or missing context, asks what you want to change, challenges assumptions, and continues the interview. Code is the source of truth. The existing Markdown file is updated only after the required decisions are made, and the agent finishes the same item ID. Refinement does not create another inbox item or erase Jira, implementation, or PR links and progress.

Once the harness accepts the prompt, SPEC becomes active. That means the session has started, not that the agent has completed its code validation. **Seal the spec** or the agent's `finish` command marks it done again while retaining downstream progress. A launch failure preserves the prior spec status and never overwrites the Markdown file; its error remains available for inspection. If workspace focusing fails after the prompt was accepted, the session stays active and its warning asks you to select the new tab manually. **Refine the spec** launches the AI interview.

CLI equivalent, run inside Herdr after setting `INBOX_ITEM_ID` to an existing item's ID:

```sh
"$HOME/git/herdr-inbox/target/release/herdr-inbox" refine "$INBOX_ITEM_ID" --profile codex
"$HOME/git/herdr-inbox/target/release/herdr-inbox" refine "$INBOX_ITEM_ID" --profile opus --topic "Tighten the retry limits"
```

`refine` accepts the same profile, model, effort, topic, and permission overrides as `launch`. Neither takes a repo: the agent finds the affected code from the spec. Ensure the real `grill-me` skill from your main AI configuration is discoverable by the chosen client; the inbox passes the skill invocation rather than installing it.

## Jira and development through agents

The Inbox never talks to Jira or writes code. It starts an agent with instructions, and the agent reports back through the CLI. Both steps need Jira turned on, a Herdr pane, and Claude with your Jira tools and skills configured.

**Forge Jira ticket** asks for an optional parent key, an epic or story such as `BT-2000`, and remembers it for next time. An agent then creates the issue from the spec under that parent, in the parent's project. The spec's own session is reused while its Claude tab is still open, since that agent already knows the spec; otherwise a new `Jira · <title>` tab starts. JIRA shows `active` until the agent reports the issue with `herdr-inbox jira ID KEY --url URL`. **Bind Jira ticket** links an existing issue by hand at any point.

Linking a ticket, by agent or by hand, renames the spec file to start with its key: `payment-retries.md` becomes `BT-2300-payment-retries.md`, in the same folder, with the item's identity and progress intact. Skills that are given only the key can find the spec by it. A name that already exists refuses the link.

**Begin dev quest** appears on DEV once a development skill is configured:

```sh
herdr-inbox settings defaults --dev-skill /team-dev    # `none` removes it
```

It opens a `Dev · BT-2300` tab and sends `/team-dev BT-2300`, followed by the spec path and the commands to report a branch (`implement`) and a draft PR (`pr`). Without a configured skill, DEV only records agent and branch as before. The CLI equivalents are `herdr-inbox ticket ID [--parent KEY]` and `herdr-inbox develop ID`.

## Workflow

```sh
~/git/herdr-inbox/target/release/herdr-inbox start "Payment retry handling"
~/git/herdr-inbox/target/release/herdr-inbox show <id>
~/git/herdr-inbox/target/release/herdr-inbox finish <id>
~/git/herdr-inbox/target/release/herdr-inbox jira <id> ABC-123 --url https://jira.example/browse/ABC-123
~/git/herdr-inbox/target/release/herdr-inbox implement <id> --agent implementor --branch feature/payment-retries
~/git/herdr-inbox/target/release/herdr-inbox pr <id> https://github.example/org/repo/pull/123
~/git/herdr-inbox/target/release/herdr-inbox archive <id> --confirm <full-id>
```

`start` remains available for creating a local record and Markdown file without launching an agent. `finish` marks the spec done and makes Jira linking available. A linked Jira ticket unlocks implementation; starting implementation unlocks draft PR linking. CLI and TUI enforce the same sequence. Existing active records retain their recorded progress. `list --json` and `show <id> --json` give agents structured state. The TUI refreshes from disk every second, so CLI updates appear there without a server.

The borderless inbox list contains only the selected folders' contents, so each spec's domain stays visible. The selected folder itself is the Inbox boundary: its immediate child folders appear at the top level, and files directly inside it remain loose items. No local metadata directory or unrelated file contributes extra rows. A waterfall tree uses branch guides and a three-cell step for each nested level. Folder, Markdown, and HTML Nerd Font icons distinguish entries. Markdown remains the default discovery format; include `**/*.html` in a source's filters to discover HTML files too. The tree reflects your own hierarchy without moving content or creating folders.

Every view shares an `Inbox` header at the same inset: two columns from the left and one row from the top. A blank row separates it from content. Settings, new/refinement sessions, and the full reader append their current location; spec details include the domain folders and title. Settings always uses `Inbox / Settings`, including first use. Source boundaries stay out of those breadcrumbs. Long ancestry shortens while preserving the `Inbox` anchor and the current location. The tree uses the popup's translucent background without a darker inset. File rows retain fixed Spec, Jira, Dev, and PR columns at the right edge; folder rows leave them blank. Long names and deep paths truncate within their column instead of shifting statuses. Below 64 columns, files show a compact `S J D P` icon trail; open a file for full status labels. Icons and words mark progress at normal widths: `○ locked`, `○ wait`, `→ ready`, `● active`, `✓ done`, and `◐ draft`; color is a secondary cue.

Hover or use `j/k` or Up/Down to select a tree row. Enter expands/collapses a folder or opens a file's detail view; clicking a folder also toggles it. Right expands a folder or enters its children; Left collapses it or selects its parent. Archive applies only to files. Folder expansion and focus survive the inbox's refreshes. The detail view shows the spec preview on the left and an interactive progress rail on the right. SPEC, JIRA, DEV, and PR share one straight vertical spine, with circular nodes, explicit states, and available ticket, branch, or PR references. SPEC has no additional session metadata. The next actionable milestone is selected when you open an item. Use `j/k` or Up/Down to select a milestone, or click its five-by-three node area. A teal ring surrounds the selected node; its center keeps the status color. Locked stages stay selectable so you can see the missing prerequisite.

Milestones keep their positions as you navigate. Every node reserves five columns by three rows for its ring, selected or idle. Rails at least 26 rows tall give every stage a fixed six-row slot with reserved control space. Shorter rails place the complete 18-column overview beside a stationary control area labeled with the selected stage. Actions and confirmation hints wrap on narrow terminals; long input scrolls horizontally to keep the cursor visible. Selection, locked guidance, text entry, and confirmations never push other milestones around. Stage labels and controls have no background fill; the selected action uses `✦` and bold, underlined teal text.

Completing an action leaves a short green acknowledgement beside its milestone, rather than in the footer. It occupies the existing context row, so the layout stays steady; compact terminals use a shorter caption. Automatic selection of the next step keeps the acknowledgement attached to the completed stage. Any manual milestone navigation or leaving the detail view dismisses it, including navigating back to the stage you just completed.

Use Tab, `h/l`, or Left/Right to cycle actions; Enter or a mouse click runs the selected flow. SPEC offers **Seal the spec**, **Read the scroll**, and **Refine the spec**; refinement opens the client chooser and starts a new AI interview for the existing spec. JIRA offers **Forge Jira ticket** and **Bind Jira ticket**, then **Visit Jira ticket** and **Update Jira link**. DEV offers **Log dev quest**, then **Update dev quest**, which record agent and branch information; with a development skill configured it offers **Begin dev quest** first, which launches that skill. PR offers **Bind draft PR**, then **Review draft PR** and **Update PR link**. A locked milestone explains its prerequisite in its control area. Input and confirmation prompts replace the selected stage's controls. When progress advances, selection follows the next recommended stage if you were on the previous recommended stage; manually selected history stays selected. Below a body width of 78 columns, the 36-column right rail moves below the spec preview. At 40 × 18 and larger, all four milestones stay visible; short terminals prioritize the progress controls.

Press `r` for the full scrollable Markdown spec; use `j/k` to scroll one line or `Shift+J/K` to scroll ten. Scrolling stops two rows after its last rendered line and adjusts to wrapping, file edits, and resizing. Press Esc to return to the previous view, or `q` to close the inbox. Spec views have no relink, editor, or archive shortcuts. Return to the list to archive with `a`, then Enter.

The list footer shares the content's two-column inset and stays on one line: `Enter open · n new · a archive · s settings`. Narrow terminals show `↵` for Enter so every action remains readable. Hover, arrow keys, or `j/k` select a row; Esc closes the inbox. Stage actions live in the detail view. Settings' footer names the selected row's action, for example `j/k select · Enter change · Esc back`. The CLI still supports creating local records without launching an agent. Status messages appear above the shortcut line. Every screen draws its shortcut line on the same row with the same unstyled text as the list; only the shortcuts themselves change.
