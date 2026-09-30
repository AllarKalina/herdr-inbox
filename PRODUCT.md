# Personal inbox

<!-- impeccable:product-schema 1 -->

## Users

Allar uses this personal Herdr inbox during software work: a grilling session produces a spec, then the work moves through a Jira ticket, agent implementation, and a draft PR. It is not a team service or a published plugin.

## Product Purpose

Keep the spec, its current stage, and the next concrete action in one place. A successful flow starts an AI spec session, preserves its Markdown output, and makes the path to a draft PR easy to follow without losing track of work in progress.

## Positioning

A terminal-native workflow inbox that launches a spec agent in Herdr and stores the resulting spec and stage record on the computer. Jira and PR links enrich that local record; no hosted inbox or sync service sits between the user and their work.

## Operating Context

- Runs as a macOS terminal popup inside Herdr, with a CLI for direct updates and agent-readable JSON output.
- Opens spec sessions in the `ai-boiler-room` workspace. The user chooses an available Claude or Codex client; each session gets its own tab, and each new item gets its own Markdown spec path.
- Refining a spec opens another client chooser and a new session tab for the existing item, title, repo, and Markdown path. The agent validates the spec against current code before continuing the grilling interview.
- A spec is named when the session is finished. The current path is spec → linked Jira ticket → implementation → linked draft PR.
- The user reads the spec and selects next actions from the inbox detail view. Keyboard operation is central to this workflow.

## Capabilities and Constraints

- Item records and inbox-owned Markdown specs live on local disk under `~/Library/Application Support/herdr-inbox/` by default. `HERDR_INBOX_HOME` can change that directory. There is no hosted database, sync service, or background daemon.
- The inbox can launch or refine a grilling session, track its spec, mark it finished, link a Jira ticket, mark implementation started, and link or open a draft PR. Jira must be linked before implementation starts.
- Refinement preserves Jira, implementation, and PR links and progress. SPEC becomes active only after the harness accepts the prompt; this is a launch status, not evidence that agent validation is complete. The agent reads the existing spec and item context, treats current code as truth, reports gaps, then interviews the user before updating the same file and finishing the same item.
- Previous launches remain in the item's local JSON history. Cancellation leaves the item unchanged; failed refinement preserves its prior spec status and does not overwrite its file. The raw editor remains available through `e`.
- Jira and PR actions currently store links to existing external work; they do not create tickets or PRs remotely. The implementation action records an agent and branch; it does not yet launch an implementer.
- Deleting an item requires a second Enter confirmation and moves inbox-owned files to the local `trash/` directory under inbox data. Linked specs outside the inbox remain in place.
- This plugin is for personal use and local linking, with no publishing step.

## Brand Commitments

Focused, playful, and personal. The progress path and next move can have a little flair, but avoid fake XP, scores, and badges. Preserve the terminal's translucent background rather than adding opaque decorative layers.

## Evidence on Hand

- The current Rust TUI, CLI, stage model, and tests are in this repository; [README.md](README.md) documents the working flow.
- The [mock grill-me skill](examples/mock-repo/.agents/skills/grill-me/SKILL.md) supports a local launch trial. It is test material, not a production spec or evidence of Jira/PR automation.
- No remote Jira-creation or implementer-launch integration exists yet; future work must not present those actions as already automated.

## Product Principles

- Show the next real action before secondary metadata.
- Keep the spec readable inside the inbox, with an easy path to its full text.
- Make prerequisites explicit: spec, Jira ticket, implementation, then draft PR.
- Keep progress legible through words as well as icons; color is a supporting cue.
- Keep the user's records and specs on local disk.

## Accessibility & Inclusion

Keyboard navigation remains complete. Icons have text labels. Animation may add flair, but no state or action depends on motion.
