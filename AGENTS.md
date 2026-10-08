# Herdr Inbox development

- Aggressive development and iteration; current implementation only.
- One source of truth: configured spec folders and their actual contents.
- No legacy support, automatic migrations, compatibility fallbacks, or workarounds preserving obsolete behavior. Remove superseded code.
- Metadata tracks workflow; it cannot add files from outside selected folders to the Inbox.
- Source files remain user-owned. Never commit or upload specs/context; keep `/specs/` ignored.
- No routine version bumps. Reserve 1.0 for Allar's complete intended feature set, fully usable and stable.
- Implement only explicitly requested changes. Ask Allar before adding features, options, or scope.
- Commits: use `/Users/allarkalina/git/codex/scripts/committer`.
