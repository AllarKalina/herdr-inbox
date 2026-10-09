# Herdr Inbox development

- Aggressive development and iteration; current implementation only.
- One source of truth: configured spec folders and their actual contents.
- No legacy support, automatic migrations, compatibility fallbacks, or workarounds preserving obsolete behavior. Remove superseded code.
- Metadata tracks workflow; it cannot add files from outside selected folders to the Inbox.
- Source files remain user-owned. Never commit or upload specs/context; keep `/specs/` ignored.
- No routine version bumps. Reserve 1.0 for Allar's complete intended feature set, fully usable and stable.
- Implement only explicitly requested changes. Ask Allar before adding features, options, or scope.
- Commits: use `/Users/allarkalina/git/ai-boiler-room/scripts/committer`.
- Gate: run `scripts/regress` after every change and before every commit or handoff; it must print `PASS`. Use `--quick` while iterating. Skill: `herdr-inbox-regress`.
- UI snapshots in `tests/snapshots/ui/` specify every screen. Re-record with `scripts/regress --update-snapshots` only for an intended UI change, review `git diff tests/snapshots`, and commit them with the change. Never re-record, loosen, or skip a check to pass.
- Read `ARCHITECTURE.md` before changing code: it maps the modules and gives the steps for adding a screen, setting, CLI command, workflow step, client, or prompt. Extend those patterns instead of adding parallel ones.
- Appearance is tested only by golden snapshots: new screen or state means a scenario in `src/ui/tests/golden/`. New or changed CLI behavior: extend `tests/cli_workflow.rs` or `tests/cli_config.rs`. Test one behaviour at one layer.
