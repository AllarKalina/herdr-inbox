# Personal inbox

<!-- impeccable:product-schema 1 -->

## Users

A Mac user connects this Herdr inbox to their own specs and context during software work: a grilling session produces a spec, then the work moves through a Jira ticket, agent implementation, and a draft PR. A public source repository distributes the plugin to any supported Mac; it is not a team service.

## Product Purpose

Keep the spec, its current stage, and the next concrete action in one place. A successful flow starts an AI spec session, preserves its Markdown output, and makes the path to a draft PR easy to follow without losing track of work in progress.

## Positioning

A terminal-native workflow inbox that launches a spec agent in Herdr and stores the resulting spec and stage record on the computer. Jira and PR links enrich that local record; no hosted inbox or sync service sits between the user and their work.

## Operating Context

- Runs as a macOS terminal popup inside Herdr, with a CLI for direct updates and agent-readable JSON output.
- Opens spec sessions in the configured workspace (default `ai-boiler-room`). The user chooses an available Claude or Codex client; each session gets its own tab, and each new item gets its own Markdown spec path.
- Refining a spec opens another client chooser and a new session tab for the existing item, title, repo, and Markdown path. The agent validates the spec against current code before continuing the grilling interview.
- A spec is named when the session is finished. The current path is spec → linked Jira ticket → implementation → linked draft PR.
- The user reads the spec and selects next actions from the inbox detail view. Keyboard operation is central to this workflow.

## Capabilities and Constraints

- Settings and item records live on local disk under `~/Library/Application Support/herdr-inbox/` by default. `HERDR_INBOX_HOME` can change that directory. There is no hosted database, sync service, or background daemon.
- The inbox can launch or refine a grilling session, track its spec, mark it finished, link a Jira ticket, mark implementation started, and link or open a draft PR. Jira must be linked before implementation starts.
- Refinement preserves Jira, implementation, and PR links and progress. SPEC becomes active only after the harness accepts the prompt; this is a launch status, not evidence that agent validation is complete. The agent reads the existing spec and item context, treats current code as truth, reports gaps, then interviews the user before updating the same file and finishing the same item.
- Previous launches remain in the item's local JSON history. Cancellation leaves the item unchanged; failed refinement preserves its prior spec status and does not overwrite its file. The raw editor remains available through `e`.
- Jira and PR actions currently store links to existing external work; they do not create tickets or PRs remotely. The implementation action records an agent and branch; it does not yet launch an implementer.
- Archiving an item with `a` requires a second Enter confirmation and moves inbox-owned files to the local `trash/` directory under inbox data. User-owned specs remain in place, regardless of location. Archived items are restorable through Settings and excluded from automatic rediscovery; the CLI retains its compatible `delete` and `restore` commands.
- This plugin is for personal use. Public GitHub installation builds the same source on each Mac; the plugin and config can be shared while inbox records and specs remain separate on local disks. Local linking remains available for development.

- Arbitrary user-selected sources discover existing Markdown recursively as Spec done. No content hierarchy or cross-computer sharing is imposed. Settings configure sources, filters, context references and launch defaults; each computer has independent metadata.
- The main inbox reflects the user's folder hierarchy as an expandable waterfall tree, making spec domains visible. Source boundaries and the legacy `Inbox specs` wrapper are omitted; real child folders start at the top level, and root-level files stay loose. Folder, Markdown, and HTML icons distinguish entries; HTML discovery uses the source's configurable include filters. Folder rows carry no workflow status and cannot be archived as items. Keyboard and mouse navigation preserve focus and expansion across refreshes.
- Every screen shares an anchored `Inbox` header and consistent inset. Breadcrumbs identify the current view or spec's domain without repeating its configured source boundary; long ancestry shortens while preserving the anchor and current location.
- The main list has one shortcut line: `Enter open · n new · a archive · s settings`. Setup, **Save and scan**, and **Restore archived item** belong in Settings; there is no separate connect-folder hint or main-list scan/restore shortcut.
- New/refinement sessions inspect explicit context paths after availability preflight. Confirmed source relocation and explicit item relinking preserve identities; unresolved sessions must be settled before rebinding.

## Brand Commitments

Focused, playful, and personal. The progress path and next move can have a little flair, but avoid fake XP, scores, and badges. Preserve the terminal's translucent background rather than adding opaque decorative layers.

## Evidence on Hand

- The current Rust TUI, CLI, stage model, and tests are in this repository; [README.md](README.md) documents the working flow.
- The real `grill-me` skill is maintained in the main AI configuration outside this plugin repository; launching a session requires it to be discoverable by the chosen client.
- No remote Jira-creation or implementer-launch integration exists yet; future work must not present those actions as already automated.

## Product Principles

- Show the next real action before secondary metadata.
- Keep the spec readable inside the inbox, with an easy path to its full text.
- Make prerequisites explicit: spec, Jira ticket, implementation, then draft PR.
- Keep progress legible through words as well as icons; color is a supporting cue.
- Keep the user's records and specs on local disk.

## Accessibility & Inclusion

Keyboard navigation remains complete. Icons have text labels. Animation may add flair, but no state or action depends on motion.
