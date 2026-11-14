# Board interactions — 2026-11-08

This pass replaces a Board that could mainly open and create cards with saved group controls, direct card movement, card actions and bounded reads. It retains the shared database menus, property pickers, page-opening modes and Phosphor icon system.

## Reference and scope

The current [Notion Board guide](https://www.notion.com/help/boards) and the existing [database video review](../research/notion/DATABASE-VIDEO-REVIEW-2026-11-08.md) are the reference sources. The official [Board tutorial](https://www.youtube.com/watch?v=_efKZce3eDY) reports column creation at 3:02–3:09, card layout at 3:23–4:27, group controls at 4:28–5:58 and subgroup movement at 5:59–6:39. The [database overview](https://www.youtube.com/watch?v=D0DFa0NbZJM) reports a Board-to-Table status change at 2:05–2:21 and group controls at 2:45–3:25.

The video notes come from scene and audio analysis. They do not measure native pixels, drag thresholds, timing or frame rate. Native computer use remains unavailable. This pass does not claim full Notion parity.

| Before | After | Why |
| --- | --- | --- |
| Cards opened pages but could not move between columns | Drag changes the typed grouping value; moving between subgroup rows changes both values in one guarded write | Keep the source data consistent with the visible destination |
| Empty columns were only creation targets | Empty and populated columns accept card drops | Support moving a card into and out of an empty-value group |
| Cards used the current generic record page | A background reader counts all matching records and reads bounded windows for each bucket | Keep counts accurate and expose records beyond the loaded page |
| Board layout had no card controls | Layout saves card size, preview, page icon, text wrapping and column color per view. Its property-order controls change the shared page property order. | Keep card display choices in the view and property order consistent across views |
| Group opened the generic property editor | Group and Sub-group have source, visibility, collapse and order controls | Make organization a view setting without rewriting unrelated fields |
| There were no card action menus | Card actions support opening, rename, move, duplicate and Trash by stable record ID | Keep actions correct after paging, filtering and reordering |
| Card properties only displayed values | Supported property controls open the shared editors or change a checkbox | Keep property actions separate from opening the card |
| Card placement had no saved manual order | Unsorted boards save card order per view and retain it through reopen and archive restore | Preserve the user's arrangement |
| Creation could conflict with a column or saved filter | Creation supplies guaranteed equality values and rejects a conflicting column before creating a record | Prevent a new card from silently receiving the wrong values |
| Rendering all column content could grow with the data | The UI uses measured card heights, spacer elements and a visible column window under one scroll area | Keep mounted content bounded without independent column scroll behavior |
| A stale setting had only an error message | Per-field guards preserve other changes, and Reload saved settings provides recovery | Keep one failed change from replacing another window's work |

## Use and saved behavior

Drag a card from its title area to another column or subgroup row. A drop within the same column sets manual order when no sort is active. With an active sort, moves between groups remain valid and the sort determines placement; manual reordering requires sorting to be removed. Escape and focus loss cancel a drag. Changing views cancels the old heading drag before it can save against the new view.

Use a card's action button or right-click menu to open, rename, move, duplicate or move it to Trash. Nested property controls act on the property and do not open the page. Source records retain their IDs after a move or rename. A duplicate receives separate record, page and block IDs; its body and appearance are copied.

Use the group header menu to create or rename a Select/Status option, collapse or hide a group, or change its order. Group rename updates the source property and matching records across the database. View-specific order, hidden and collapsed state remain separate from record values.

View settings → Layout contains card size, preview, page-icon, wrapping, color and property-order controls. The shared Properties and Open pages in controls remain available. Group and Sub-group select distinct supported properties and expose saved visibility, collapse and order. A missing grouping property has a repair action.

New supplies the current view's guaranteed filter values. A column footer also supplies its group and subgroup values. Conflicting context is rejected before a record is created. New cards append in an unsorted board. Ambiguous OR filters do not cause an arbitrary branch to be selected.

## Bounds and remaining limits

Each populated group/subgroup bucket reads a 50-card window, with Previous and Next controls. The reader limits the group and subgroup value sets to 128 and 32, with empty-value groups handled separately, and keeps at most 256 populated bucket windows. It still counts the full matching source and retains full totals for the represented group values when bucket windows are capped. A truncation message asks for a narrower filter when more groups match.

The reader streams source records in the background. It does not place an entire large database in the UI. The UI renders visible cards and columns with overscan and spacers. These bounds are not a measured frame-rate or first-viewport result for a million-row Board.

Current grouping supports editable scalar properties: Status, Select, Text, Number, Checkbox, Date, URL, Email and Phone. Numeric group values normalize equivalent integer and floating-point representations. Multi-select, person, relation and computed grouping remain outside this pass. Card previews support none, page content and the saved page cover; additional file-property preview sources and image-fit/reposition controls are not claimed. Multi-card selection, bulk dragging and unified drag Undo are not established by these checks.

## Validation status

All nine new runtime interaction tests pass. They cover cross-group and empty-group drops, Table/reopen consistency, subgroup movement, same-column order, Escape/focus cancellation, nested property actions, card rename/duplicate/Trash, group creation/rename/collapse/hide, and a 125-record group read in 50/50/25 windows with bounded mounted nodes. They also cover 20 columns × 10 subgroups with both scroll axes, a heading drag interrupted by a view switch, and filter-aware toolbar/footer creation with append order and conflict recovery. The earlier 48-test app Board filter and the record-icon body-content regression also passed.

The full workspace suite passes 427 tests: 314 app, 100 core and 13 engine. Three probes are ignored in that suite; the public map fixture probe was also run separately and passed. Formatting, diff checks and the debug build pass. Light/1× and dark/2× Board captures were inspected, with separate checks for content and cover previews, card actions and saved layout controls. Local captures are under `/tmp/moss-board-review/`.

Tests use Rune runtime pointer and keyboard events with local synthetic data. They do not replace native pointer, trackpad, accessibility, IME or frame-timing checks. No private reference capture is included in this document.

## Badge and drag repair — 2026-11-08

The reported failure exposed gaps in the earlier test paths. Those tests started on the title and dropped inside a content-sized bucket. They also forced extra redraws after each read, which hid a layout-settlement fault.

| Before | After | Why |
| --- | --- | --- |
| A status badge grew to fill its property row | The badge fits its dot, text and padding | Keep a short status label compact |
| Property rows captured clicks and blocked card drags | Non-editing property rows forward drag events and suppress the click after a move | Keep click-to-edit and let users drag from the card body |
| Drop targets used rectangles captured during view construction | Drop targets read the latest drawn column, lane and card rectangles at the event | Keep drop positions current after records or layout change |
| Only the area occupied by cards accepted a drop | The column heading, New page row and space below cards accept a drop within their lane | Match the visible column target without crossing subgroup boundaries |
| A view switch could reset the drop viewport to zero without another layout report | Valid viewport geometry survives a context reset | Allow a drag after switching views with the same layout |
| The card wrapper key used its index in the loaded record page | The wrapper key uses the permanent record ID | Keep pointer capture through a background record reorder and deliver release or cancel |

The interaction tests no longer force extra redraws to prepare drop targets. New regressions cover badge width at 1×/2×, dragging from a property, footer and whitespace drops, Table-to-Board switching, and a loaded-record reorder during an active drag. Existing checks retain subgroup movement, manual order, cancellation, editing and paging. Local synthetic renders are under `/tmp/moss-board-repair-review/`. The computer-use host still fails with `Sky Computer Use native pipe startup failed`; native physical input remains unverified.

The repaired workspace passes 432 tests: 319 app, 100 core and 13 engine, with three probes ignored. Formatting, diff checks and the debug build pass. The badge regression checks the measured text width plus dot, gap and padding. The drag helper checks that the source path stays stable, a destination is found, release clears the drag, and no write error occurs.

All 14 Board interaction tests also passed in ten consecutive runs. Light/1× and dark/2× renders were inspected with status and date properties visible. These repeated runtime checks do not establish native physical-input or frame-rate results.

## Drag preview outside the Board — 2026-11-08

| Before | After | Why |
| --- | --- | --- |
| The raised card still inherited the Board scroll clip | An inert preview draws at the window root, while the source card stays transparent in its slot | Let the card cross the Board edge without losing pointer capture or changing layout |
| The drag offset followed a source that could move during scrolling | The preview uses the source position at pointer-down plus the pointer delta | Keep the same grab point during edge scrolling |

The boundary regression drags across all four Board edges at 1× and 2×. It checks the preview's full unclipped rectangle, stable source slot, inert input behavior and cancellation without a write. The 58-test app `board_` filter passes, including the 15 Board interaction tests. A synthetic render at `/tmp/moss-board-repair-review/unclipped-drag.png` was inspected with the preview crossing the Board edge into the sidebar area. Build, formatting and diff checks pass. This concerns the Board's clipping boundary; the native window still limits what can be displayed.

## Footer spacing and lifted cards — 2026-11-08

| Before | After | Why |
| --- | --- | --- |
| Empty columns reserved 48 points above New page | New page sits six points below the heading or last card | Keep the action next to the column's content |
| A transparent source card still occupied its full layout slot | The source stays mounted outside normal layout while the drag preview moves | Close the source gap without losing pointer capture |
| A growth stack retained old drawn row positions after layout changed | The existing measured card windows use normal column layout | Keep cards and the footer aligned during a drag |

Only the current drop marker adds space. Cancel restores the source order, while a saved move leaves no return slot. A dragged source column stays mounted during horizontal edge scrolling. The new regression checks footer spacing in an empty column, removal of the first and last source cards, cancellation and a saved move at both display scales. The subgroup test aims at its current row after source removal moves the rows below it.

The 59-test app `board_` filter passes, including all 16 Board interaction tests. Build, formatting and diff checks pass. Rest and lifted-card renders under `/tmp/moss-board-flow-review/` were inspected. The development app was restarted. Native input remains unverified: computer use can list apps after the restart, but it rejects the unbundled development executable as an invalid app.
