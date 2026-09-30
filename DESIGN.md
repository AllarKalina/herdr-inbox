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
    textColor: "{colors.teal}"
    typography: "{typography.terminal-emphasis}"
    textDecoration: underline
  breadcrumb-current:
    textColor: "{colors.teal}"
    typography: "{typography.terminal-emphasis}"
  progress-path:
    textColor: "{colors.subtext-zero}"
    typography: "{typography.terminal}"
  milestone-selected:
    ringColor: "{colors.teal}"
    backgroundColor: transparent
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

The inbox is a compact terminal ledger with just enough quest language to make real progress feel visible. One continuous canvas carries the list, spec, milestones, and next move. A teal ring identifies the inspected milestone; a small star and underlined action identify its next move. The list retains its bright selected row while the detail view stays unfilled and quiet enough for reading.

The current personal setup pairs the Rust TUI with Catppuccin Mocha in Ghostty and Herdr. Ratatui emits terminal color roles, not these hex values directly. The frontmatter records how those roles resolve in Allar's current Ghostty profile; another terminal theme may change their appearance. The terminal owns the translucent background and font, while the inbox owns layout, hierarchy, and state styling.

**Key Characteristics:**

- Flat, transparent-feeling canvas with no decorative inset card.
- Monospaced, cell-aligned information with a flexible name column and fixed status columns.
- Teal milestone rings and starred actions, semantic status colors, and words beside every icon.
- A straight vertical milestone path whose selected stage owns the nearby actions, without points or badges.

## Colors

The active Ghostty profile supplies the Catppuccin Mocha palette. Its source is `~/git/ghostty-herdr-config/ghostty/config.txt`, with palette values in `~/.config/ghostty/themes/catppuccin-mocha.conf`. In Rust, continue to use Ratatui's terminal colors; the frontmatter values document this profile's current rendering.

### Primary

- **Teal:** the selected inbox row background, current breadcrumb title, active progress, selected milestone ring, and selected action text. Dark Surface One text sits on the list selection; detail controls keep the transparent canvas.

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

The detail view has a two-cell horizontal and one-cell vertical outer inset. A breadcrumb sits above the main content. At a body width of 78 columns, the spec takes the left side and the interactive `PROGRESS` rail occupies 36 columns on the right, separated by a two-column gap. Below that breakpoint, the rail moves under the spec. The spec preview is capped at 86 columns. Long titles shorten to preserve the final breadcrumb.

Milestone centers share column seven and begin two rows below `PROGRESS`. Every node reserves five columns by three rows, including an idle node, so selecting it cannot move its neighbors. A rail at least 26 rows tall gives each stage a fixed six-row slot, with context, status words, and controls beginning at column 11. Empty control space remains reserved when another stage is selected. Shorter rails keep all four nodes in a fixed 18-column overview, a two-column gap, and a stationary control area starting at column 20. That area identifies the selected stage, reserves four context rows beginning three rows below `PROGRESS`, and begins actions seven rows below `PROGRESS`; narrow labels and hints wrap. SPEC leaves its context blank: harness, space, session, and redundant completion metadata do not belong in this rail. At 40 × 18 and larger, all four milestones remain visible. Selecting a stage, inspecting a prerequisite, or opening a prompt never moves the milestone nodes, spec preview, or footer.

The full reader replaces the detail content with the document. Its breadcrumb keeps `Inbox / item / FULL SPEC`; the text scrolls only to the last rendered line plus two blank rows. A blank row separates the document from the footer hints: one-line `j/k` scrolling and ten-line `Shift+J/K` jumps.

## Elevation & Depth

The inbox has no shadow vocabulary. Depth comes from the host terminal's translucent surface (Ghostty opacity `0.95` and blur setting `20`), the selected list highlight, a teal milestone ring, and occasional structural borders on choice or confirmation prompts. Regular list and detail content stays on the same background plane.

**The Flat Ledger Rule.** Use spacing, alignment, text weight, and selection state before adding a border. Reserve a full border for a prompt that interrupts the normal flow.

## Shapes

The TUI uses rectangular cell geometry, with no corner-radius tokens. Circular milestone nodes share a single vertical `│` spine; their labels and states align beside it. The selected node gains a teal ring drawn as a five-column, three-row Braille Canvas circle around its semantic status center. Stage labels stay unfilled. Selected actions use `✦` with bold, underlined teal text; other actions use blue text with two leading blank cells. No arrows or opaque button fills are added. The standard inbox list and detail view have no surrounding card border. Client-choice prompts use a plain rectangular border; milestone prompts occupy the existing reserved control area.

## Components

### Inbox table

The row title leads; Spec, Jira, Dev, and PR statuses stay aligned at the right. A selected row uses a teal fill, Surface One text, and bold weight. Unselected status cells use semantic foreground colors. Hover or keyboard movement changes selection without changing the column grid.

### Breadcrumb and spec

The breadcrumb retains the full path while coloring only the current location teal. `SPEC` introduces a text preview; opening the full reader gives the Markdown its own scrollable surface. The preview and progress rail share a top edge on wide terminals.

### Milestone navigation

SPEC, JIRA, DEV, and PR sit on one straight vertical spine. Each circular node has an adjacent label and explicit state, so completed, ready, active, and locked stages can be scanned together. Selection is independent of completion: the teal ring identifies the stage being inspected while its semantic center and state remain readable. The complete five-by-three node area is clickable.

Opening an item selects its next actionable milestone. Use `j/k` or Up/Down to move through the four milestones; a mouse click selects a stage. When the recommended stage advances, selection follows it only if the previous recommended stage was selected; a manually selected earlier stage stays selected. Locked stages can be inspected and explain their prerequisite. A linked Jira ticket is a hard dependency for starting implementation; implementation unlocks draft PR linking.

### Milestone actions

On roomy rails, actions stack beneath the selected milestone in its reserved control area; context is shown for Jira, development, and PR only. The spine continues beside the controls. Other stages keep exactly the same spacing, whether selected or idle. On compact rails, the selected stage heading and controls occupy a stationary area beside the complete milestone overview. This keeps controls associated with their stage without moving the path. The first action is the primary next move; `Tab`, `h/l`, or Left/Right cycle available actions, and Enter runs the selected action. Clicking an action runs it.

SPEC offers **Seal the spec**, **Read the scroll**, and **Refine the spec**. JIRA offers **Bind Jira ticket**, then **Visit Jira ticket** and **Update Jira link**. DEV offers **Log dev quest**, then **Update dev quest**; these record the agent and branch and do not launch an implementer. PR offers **Bind draft PR**, then **Review draft PR** and **Update PR link**. Labels add a personal quest motif without implying nonexistent automation. Locked stages explain the missing prerequisite in their control area instead of offering an enabled progression action. Success copy celebrates real transitions, such as a sealed spec and unlocked Jira step. One blank row separates the content from the detail footer, whose shortcut line stays `j/k stage · Tab action · r read` at every size. Enter still runs the selected action; Esc still returns to the list.

**Refine the spec** opens a separate installed-client chooser view, retaining the `Inbox / item / Refine` breadcrumb, then starts a new grilling session in `ai-boiler-room` for the same item and Markdown path. The title and repo stay attached to that item. The agent reads its spec and context, validates claims against current code, reports gaps, then asks what to change and continues the interview. Only agreed decisions update the existing file. SPEC becomes active after the harness accepts the prompt; do not present that transition as completed agent validation. Sealing again preserves Jira, development, and PR progress. A cancelled chooser makes no change; a failed launch preserves the prior spec status and file. Earlier launches remain in local JSON history. The `e` shortcut still opens the raw editor rather than the refinement interview.

### Prompts

Text entry and deletion confirmation replace the selected milestone's controls in the same reserved area. Input scrolls horizontally to keep its trailing cursor visible. Narrow action labels and confirmation hints wrap within the control area. A confirmation names the target, explains what moves to local Trash, and gives explicit Enter and Esc outcomes. All milestone nodes retain their positions throughout entry, confirmation, and cancellation.

## Do's and Don'ts

### Do:

- **Do** preserve the host's transparent background and terminal font.
- **Do** use teal to connect the list selection, milestone ring, and starred action.
- **Do** keep all milestone nodes on one spine and place actions with their selected stage.
- **Do** reserve control space so navigation and prompts leave the layout steady.
- **Do** align status columns even when a spec title is long.
- **Do** show locked prerequisites and a readable label beside every status icon.

### Don't:

- **Don't** introduce opaque full-screen fills, nested cards, or decorative shadows.
- **Don't** imply progress with fake XP, points, or badges.
- **Don't** hard-code Ghostty's hex values into the Rust UI; use terminal color roles so the TUI remains usable in another terminal.
