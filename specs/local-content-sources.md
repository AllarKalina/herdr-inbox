---
read_when:
  - Implementing Inbox settings, spec discovery, or metadata storage.
  - Changing how existing user-owned specs enter the Inbox.
---

# User-owned content sources and local Inbox metadata

Status: design spec. No implementation changes yet.

## Goal

Any supported Mac can install the same Herdr Inbox plugin and connect it to that user's existing specs and context. Each computer has independent content, settings, and metadata. The plugin provides the workflow and presentation layer over user-owned content.

## Agreed requirements

- macOS-native storage and behavior.
- Settings: `~/Library/Application Support/herdr-inbox/settings.toml`.
- Metadata: `~/Library/Application Support/herdr-inbox/items/<id>.json`.
- Specs and context stay in arbitrary user-selected locations. No required directory tree, filenames, repository layout, or Work/Personal environment model.
- Selecting a specs folder automatically discovers existing specs, including subfolders.
- Newly imported specs start in **Spec done** state. Jira is ready to link; Dev and PR remain locked until their existing prerequisites are satisfied.
- Existing records retain their current progress. Import does not mark an existing in-progress record done or overwrite enriched metadata.
- The user will create a separate script using Jira and Git MCPs to enrich records on the other computer. Building that script or adding MCP integrations to the plugin is outside this change.
- No cross-computer synchronization, shared inbox service, or transfer of user content or metadata through the plugin installation/config repositories.

## Current behavior and affected code

- `src/store.rs:121`: default storage is already the macOS Application Support directory; `HERDR_INBOX_HOME` overrides it.
- `src/store.rs:26`: one item record stores workflow state, absolute spec/repo paths, and local launch history.
- `src/store.rs:182`: creation assigns a UUID and normally creates a spec under the application data directory; `--spec` can reference an external file.
- `src/store.rs:141`: ownership is currently inferred from the absolute managed-spec path.
- `src/store.rs:150` and `385`: local file locking and atomic record replacement protect updates.
- `src/store.rs:270`: finishing a spec sets it to done and makes Jira ready to link.
- `src/main.rs:168`: `jira`, `implement`, and `pr` update workflow links through validated operations. `list --json` and `show ID --json` expose records to scripts.
- `src/launch/prompts.rs`: launch prompts pin the item ID, exact spec path, Inbox data directory, and finish command.
- `src/ui/detail.rs`: the UI reads the referenced Markdown file directly.
- There is no settings file, source registry, or folder-discovery flow today.

## Storage responsibilities

### Settings

Use a human-readable, versioned TOML file. The settings UI and CLI must use the same configuration reader/writer.

Proposed settings fields:

- Schema version.
- Spec sources: a stable local source ID, user-selected folder, recursive discovery option, and configurable include/exclude rules.
- Context references: user-selected files, folders, or repository directories. These are references for agent context, not another class of automatically imported spec.
- Launch defaults such as preferred available client and Herdr workspace.

Configured context references are supplied as explicit local paths to new and refinement sessions, alongside the exact selected spec path. The agent inspects relevant references; the plugin does not copy context content into a second store. Validate reference availability before starting a session. Report missing references and let the user correct/remove them or cancel; do not silently omit them or advance workflow progress on a failed preflight.

Folders must be supplied by the user. Source IDs are internal metadata; users do not need to add IDs or manifests to their content folders. Settings are local to the computer and are not included in the shared terminal-config installer.

Keep `HERDR_INBOX_HOME` as a compatible override for the local data root; settings and metadata should resolve under that root when overridden so tests and existing automation remain isolated.

### Metadata

Retain one JSON record per item initially. Add source association and explicit file ownership/reference semantics without replacing user content with an application copy.

Each discovered spec receives a stable UUID. Track its source and location separately from that UUID. An explicitly confirmed relocation of the same source can preserve its source ID and associated item IDs. Selecting an unrelated replacement folder creates a new source; matching relative filenames alone must not transfer another spec's identity or progress. Metadata may record a source-relative path plus the resolved local spec path for compatibility with existing CLI consumers.

Workflow data includes title, spec status, Jira link, implementation information, PR link, timestamps, and local launch history. Generated prompts and terminal handles remain local execution information. Metadata never needs to travel to another computer.

## Setup and discovery

1. On first use, show settings when no content sources have been configured.
2. Let the user select their specs folder and optional context references.
3. Validate and save the local settings.
4. Scan matching files and create metadata for previously unknown specs.
5. Show discovered specs in the Inbox, with an import count and actionable errors for inaccessible sources/files.

The scanner must be reusable from the settings flow, a CLI scan operation, and a Rescan action. Installation may invoke setup, but must not guess a user's content folders or index arbitrary directories during plugin builds.

Use the existing Markdown reader for the initial implementation. Proposed default discovery matches `.md` and `.markdown` recursively; users can change filters. This is a content-format filter, not a required folder layout. Context files need not follow those spec filters.

Derive an initial title from the first Markdown H1, falling back to the filename. Do not rename or edit the file. New import defaults:

- Spec: `done`.
- Jira: `ready`, with no inferred key or URL.
- Implementation: `waiting` (displayed as locked until Jira is linked).
- PR: `waiting` (displayed as locked until prerequisites are met).
- Launch history: empty.

Do not infer Jira, Git, or PR state from filenames or content. The user's enrichment script supplies those facts.

## Repeat scans and file lifecycle

- Repeated scans are idempotent: unchanged files retain item IDs, titles, timestamps, progress, and links; no duplicate records.
- New matching files appear on the next scan. Scan configured sources on Inbox opening and when settings are saved; Rescan handles changes while the Inbox is open.
- An unavailable root or missing file does not delete metadata. Keep the record and report that its source/spec is unavailable.
- Overlapping roots and aliases must not create multiple items for the same resolved file.
- Scanner-created records explicitly reference user-owned files. Removing a record must not remove its user-owned spec or context files. Keep recoverable metadata in local Trash and suppress that source/spec reference from automatic discovery until explicit Restore/Reimport. Restoring reuses its existing UUID and enriched progress; delete followed by reopening must not recreate the record.
- If a rename or move cannot be identified unambiguously, preserve existing metadata and allow explicit relinking. Do not merge records merely because their content is identical.
- Active agent sessions retain their resolved source, spec path, item ID, and data root. Settings changes apply to future launches. Block source relocation or item relinking while an affected spec session is unresolved; completing or explicitly settling the session must precede rebinding. Historical launch handles alone are not proof of a live session. Do not redirect a running agent's finish command to another spec/store.
- Creation and explicit refinement can write to the exact user-selected destination. Scanning itself is read-only with respect to source content.

## Script interface

Document the item JSON fields and existing CLI operations so the user's MCP script can:

1. Enumerate local records using `list --json`.
2. Correlate records with their spec files and obtain Jira/Git information externally.
3. Update the correct UUID through `jira`, `implement`, and `pr` in prerequisite order.
4. Inspect results through `show ID --json`.

The existing mutation commands cover Jira links, implementation branches/agents, and PR links. They do not update an existing item's checkout path; add a documented update operation only if that becomes necessary for the user's script.

These updates must use the same validation, local lock, and atomic writes as the UI. Rescanning must preserve script-supplied progress and links. No Jira tickets, branches, or PRs are created by importing a spec.

## Migration and compatibility

- Preserve existing item IDs, paths, titles, workflow links, progress, and launch history.
- Preserve existing spec files at their current locations. Do not move user content during setup or migration.
- Recognize already-tracked files before importing newly discovered specs.
- Existing Inbox-generated specs retain their legacy ownership information; newly discovered files are user-owned. Replace path-based ownership guessing with explicit metadata.
- Version settings and any new metadata schema. Back up existing metadata before a destructive schema migration; reject unsupported future schema versions without writing them.
- Update README and PRODUCT.md when implemented. They currently describe application-generated spec paths and separate local inboxes without configurable discovery.

## Acceptance criteria and validation

- Two Macs can select entirely unrelated folders and operate independently with identical plugin code.
- Nested specs with arbitrary filenames appear without moving files, adding sidecars, or imposing a directory tree.
- New imports show Spec done and the correct downstream prerequisites.
- Rescanning after CLI enrichment preserves UUIDs and Jira/implementation/PR data.
- Existing in-progress records are not silently completed by discovery.
- Missing/unreadable sources preserve existing records and produce useful feedback.
- Confirmed relocation preserves local item identity; an unrelated replacement folder never inherits progress solely through matching filenames. Ambiguous moves can be explicitly relinked.
- Removing imported metadata leaves source files intact and does not resurrect the item on rescan; explicit restore preserves its UUID and enrichment.
- Changing settings does not alter active launch targets; relocation/relinking is blocked until affected spec sessions are settled.
- New and refinement prompts receive configured context paths. Missing references produce actionable preflight feedback without partial workflow changes.
- Migration preserves legacy records and external specs without duplicate imports.
- Settings persist across restarts and plugin upgrades; plugin builds/install updates never overwrite local settings or metadata.
- Add meaningful storage/discovery/settings integration tests with isolated temporary roots. Run the repository's Rust test, formatting, and lint gates during implementation; do not launch live Herdr sessions in these tests.

## Remaining implementation decisions

- Exact settings-screen navigation and folder-entry controls.
- New-spec filename/destination selection UI; it must honor user-selected locations without mandatory subdirectories.
- Filter syntax and precise handling of symlink traversal.
- Explicit relinking UI and scanner performance strategy for large folders.
- Older-binary writer compatibility during any metadata schema migration.

These details do not change the agreed local storage, user-owned content, or imported Spec done behavior.
