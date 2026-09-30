---
name: Personal Inbox
description: A crisp, lightly playful terminal quest ledger for specs moving toward a draft PR.
colors:
  mocha-base: "#1e1e2e"
  surface-one: "#45475a"
  surface-two: "#585b70"
  subtext-zero: "#a6adc8"
  blue: "#89b4fa"
  teal: "#94e2d5"
  green: "#a6e3a1"
  yellow: "#f9e2af"
  red: "#f38ba8"
typography:
  terminal:
    fontFamily: "JetBrainsMono Nerd Font Mono"
    fontSize: "16px"
    fontWeight: 600
  terminal-emphasis:
    fontFamily: "JetBrainsMono Nerd Font Mono"
    fontSize: "16px"
    fontWeight: 700
spacing:
  list-inset-cells: 1
  detail-inset-columns: 2
  detail-inset-rows: 1
  column-gap-cells: 2
components:
  inbox-row-selected:
    backgroundColor: "{colors.teal}"
    textColor: "{colors.surface-one}"
    typography: "{typography.terminal-emphasis}"
  action-selected:
    backgroundColor: "{colors.teal}"
    textColor: "{colors.surface-one}"
    typography: "{typography.terminal-emphasis}"
  breadcrumb-current:
    textColor: "{colors.teal}"
    typography: "{typography.terminal-emphasis}"
  progress-path:
    textColor: "{colors.subtext-zero}"
    typography: "{typography.terminal}"
  milestone-selected:
    backgroundColor: "{colors.teal}"
    textColor: "{colors.surface-one}"
    typography: "{typography.terminal-emphasis}"
  confirmation-panel:
    textColor: "{colors.blue}"
    typography: "{typography.terminal}"
  status-active:
    textColor: "{colors.teal}"
  status-ready:
    textColor: "{colors.blue}"
  status-done:
    textColor: "{colors.green}"
  status-draft:
    textColor: "{colors.yellow}"
  status-locked:
    textColor: "{colors.surface-two}"
---

# Design System: Personal Inbox

## Overview

**Creative North Star: "The Quest Ledger"**

The inbox is a compact terminal ledger with just enough quest language to make real progress feel visible. One continuous canvas carries the list, spec, milestones, and next move. A bright selected row or action identifies the current target; the rest stays quiet enough for reading.

The current personal setup pairs the Rust TUI with Catppuccin Mocha in Ghostty and Herdr. Ratatui emits terminal color roles, not these hex values directly. The frontmatter records how those roles resolve in Allar's current Ghostty profile; another terminal theme may change their appearance. The terminal owns the translucent background and font, while the inbox owns layout, hierarchy, and state styling.

**Key Characteristics:**

- Flat, transparent-feeling canvas with no decorative inset card.
- Monospaced, cell-aligned information with a flexible name column and fixed status columns.
- A mint-teal selection, semantic status colors, and words beside every icon.
- A straight vertical milestone path whose selected stage owns the nearby actions, without points or badges.

## Colors

The active Ghostty profile supplies the Catppuccin Mocha palette. Its source is `~/git/ghostty-herdr-config/ghostty/config.txt`, with palette values in `~/.config/ghostty/themes/catppuccin-mocha.conf`. In Rust, continue to use Ratatui's terminal colors; the frontmatter values document this profile's current rendering.

### Primary

- **Teal:** the selected row or action background, the current breadcrumb title, and active progress. Dark Surface One text sits on a teal selection.

### Secondary

- **Blue:** ready milestones and unselected actions. Ghostty also uses Blue as its configured default foreground.
- **Green:** completed milestones and success messages.
- **Yellow:** draft PRs and unknown states. **Red:** failures.

### Neutral

- **Mocha Base:** the terminal canvas behind the popup, not an additional fill painted by the inbox.
- **Subtext Zero:** secondary labels, column headers, breadcrumbs, and shortcuts.
- **Surface Two:** locked stages and the progress spine. **Surface One:** foreground on teal selections.

**The Host Owns the Canvas Rule.** Never paint an opaque full-screen rectangle merely to recreate Mocha Base. The existing Ghostty profile uses a translucent background; Herdr's panel background is reset.

**The Status Pair Rule.** Pair each semantic color with its existing icon and word, such as `✓ done`, `→ ready`, or `○ locked`.

## Typography

**Terminal Font:** JetBrainsMono Nerd Font Mono, set by Ghostty at the size and base weight in the frontmatter. The inbox does not load a font of its own.

All content shares one terminal cell grid. Uppercase labels such as `SPEC` and `PROGRESS` establish hierarchy without changing size. Bold distinguishes selected rows, milestones, actions, and the current breadcrumb; spec text stays readable at the terminal's normal weight. The spec preview and full reader display Markdown as text rather than rendering a second type system.

**The One Grid Rule.** Keep labels, values, icons, and milestone nodes aligned to terminal cells. Do not introduce proportional type inside the TUI.

## Layout

The list uses a flexible name column followed by four fixed eight-cell status columns, with two cells between columns. Its content and shortcut line sit one cell inside the popup edge. Long names truncate within their own column rather than moving statuses. Below 64 columns, the statuses become a compact, colored `S J D P` icon trail so the title remains readable; the detail view retains the full words.

The detail view has a two-cell horizontal and one-cell vertical outer inset. A breadcrumb sits above the main content. At a body width of 78 columns, the spec takes the left side and the interactive `PROGRESS` rail occupies 36 columns on the right, separated by a two-column gap. Milestone nodes share column seven; context and controls begin at column nine. The selected milestone expands inline: its context, then its action buttons, then the next node. The connecting spine continues through this expansion. Below that breakpoint, the compact rail moves under the spec. At 40 × 18 and larger, all four milestones remain visible; short terminals reduce the space between nodes and prioritize the rail. The spec preview is capped at 86 columns. Long titles shorten to preserve the final breadcrumb.

The full reader replaces the detail content with the document. Its breadcrumb keeps `Inbox / item / FULL SPEC`; the text scrolls only to the last rendered line plus two blank rows. Footer guidance is short: one-line `j/k` scrolling and ten-line `Shift+J/K` jumps.

## Elevation & Depth

The inbox has no shadow vocabulary. Depth comes from the host terminal's translucent surface (Ghostty opacity `0.95` and blur setting `20`), the selected teal highlight, and occasional structural borders on choice or confirmation prompts. Regular list and detail content stays on the same background plane.

**The Flat Ledger Rule.** Use spacing, alignment, text weight, and selection state before adding a border. Reserve a full border for a prompt that interrupts the normal flow.

## Shapes

The TUI uses rectangular cell geometry, with no corner-radius tokens. Circular milestone nodes share a single vertical `│` spine; their labels, states, and context align beside it. Only the selected milestone's label takes a teal fill, preserving the node and state's semantic colors. Selected actions use a teal button highlight. The standard inbox list and detail view have no surrounding card border. Client-choice prompts use a plain rectangular border; milestone prompts expand inline.

## Components

### Inbox table

The row title leads; Spec, Jira, Dev, and PR statuses stay aligned at the right. A selected row uses a teal fill, Surface One text, and bold weight. Unselected status cells use semantic foreground colors. Hover or keyboard movement changes selection without changing the column grid.

### Breadcrumb and spec

The breadcrumb retains the full path while coloring only the current location teal. `SPEC` introduces a text preview; opening the full reader gives the Markdown its own scrollable surface. The preview and progress rail share a top edge on wide terminals.

### Milestone navigation

SPEC, JIRA, DEV, and PR sit on one straight vertical spine. Each circular node has an adjacent label and explicit state, so completed, ready, active, and locked stages can be scanned together. Selection is independent of completion: the teal highlight identifies the stage being inspected while its state remains readable.

Opening an item selects its next actionable milestone. Use `j/k` or Up/Down to move through the four milestones; a mouse click selects a stage. When the recommended stage advances, selection follows it only if the previous recommended stage was selected; a manually selected earlier stage stays selected. Locked stages can be inspected and explain their prerequisite. A linked Jira ticket is a hard dependency for starting implementation; implementation unlocks draft PR linking.

### Milestone actions

The selected milestone expands directly beneath its own context, before the next milestone. Its actions stack as individual buttons aligned with the context; the spine continues beside them. Other milestones retain their compact overview, so the actions belong visibly to one stage. The first action is the primary next move; `Tab`, `h/l`, or Left/Right cycle available actions, and Enter runs the selected action. Clicking an action runs it.

SPEC offers reading and editing alongside finishing an unfinished spec. JIRA offers linking when ready, then opening and updating a linked ticket. DEV offers starting implementation when ready, then updating its details while implementation is active or a draft exists. PR offers linking when ready, then reviewing and updating the draft PR. Locked stages explain the missing prerequisite locally beneath their label instead of offering an enabled progression action. Completed stages retain useful read, open, edit, or update actions. Feedback and compact navigation hints remain in the view footer: `j/k stage · Tab action · Enter act · r read · Esc back`.

### Prompts

Text entry and deletion confirmation replace the selected milestone's button rows in place. Input scrolls horizontally to keep its trailing cursor visible. A confirmation names the target, explains what moves to local Trash, and gives explicit Enter and Esc outcomes. The remaining milestone nodes stay visible to preserve orientation.

## Do's and Don'ts

### Do:

- **Do** preserve the host's transparent background and terminal font.
- **Do** keep selected rows and actions visually consistent.
- **Do** keep all milestone nodes on one spine and place actions with their selected stage.
- **Do** align status columns even when a spec title is long.
- **Do** show locked prerequisites and a readable label beside every status icon.

### Don't:

- **Don't** introduce opaque full-screen fills, nested cards, or decorative shadows.
- **Don't** imply progress with fake XP, points, or badges.
- **Don't** hard-code Ghostty's hex values into the Rust UI; use terminal color roles so the TUI remains usable in another terminal.
