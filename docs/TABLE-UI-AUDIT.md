# Table new-row UI audit

Read-only source review, 7 November 2026. No app code or user data was changed. The reference is Notion 7.42.3 deep captures `009-table-inline-blank` and `010-table-tab-from-title`, plus the Table creation observations in `research/notion/CREATE-BEHAVIOR.md`.

## Finding

In `crates/moss-app/src/ui/database.rs:234`, an active cell returns a separate `text_input` instead of retaining the normal cell row. The resting cell at lines 243–258 has a 34-unit height, a 0.5-unit grid border, 9-unit horizontal padding, and `center_y()` around the label. The editor at lines 193–205 has the same nominal height and width but a 1-unit blue border, 8-unit padding, and no equivalent centered row wrapper. Thus the active cell does not share the resting grid element or its vertical alignment. That is a concrete cause of the different border and layout path. The user-supplied 18:57 cropped Moss screenshot shows the editing text displaced below the bordered row. It confirms a visible defect, but does not establish the exact displacement or prove which layout difference causes it. Rune's `text_input` starts as a text element; its final baseline depends on Rune layout and text metrics.

The whole row is assembled from separate cells at lines 294–307. The other property cells are still rendered, but the active cell replaces its own grid border with blue. The Notion blank-row capture shows the focused title overlay while adjacent cells and their grid remain visible. The reference analysis found approximately 38 logical points between ordinary single-line table boundaries; Moss currently specifies 34 units for both rows and headers. Treat the 38-point comparison as approximate, not an exact measurement for every row or configuration.

## Creation and keyboard paths

- Table footer `+ New page` calls `create_inline(..., false)` (`database.rs:310`). `create_item` creates an empty persisted record, applies the first configured Status option when present, and appends it to the current loaded records (`records.rs:43–75`). `create_inline` then focuses the blank title field (`records.rs:83–96`). This matches the observed inline creation shape, subject to sort/filter placement and actual field geometry.
- Toolbar `New` calls `add_record`, which calls `create_peek` (`database.rs:72`, `records.rs:40–42, 98–104`). This follows the observed split between footer inline creation and toolbar page peek.
- The editor saves on input and Return invokes `finish_cell(true)` (`database.rs:207–213`). Footer creation passes `repeat=false`, so `repeat_values` is absent and Return saves and ends editing without creating another row (`records.rs:318–333`). The Notion Table observation also reports Return commits and exits inline editing.
- The editor explicitly handles Escape, but not Tab (`database.rs:215–221`). Rune routes an unhandled Tab through general focus traversal; Moss's blur poll finishes the edit (`records.rs:440–446`). There is no table-specific code to select the next property cell. Notion `010-table-tab-from-title` visibly shows the Date cell selected after Tab. The actual Moss destination depends on Rune focus traversal and macOS focus settings; it was not measured in this audit.

## Recommended verification and change

| Before | After | Why |
| --- | --- | --- |
| An input replaces the resting cell wrapper. | Keep one cell wrapper and put the input inside it. Draw focus inside the wrapper. | Keep grid borders and column boundaries stable. |
| Resting and active text use different alignment and padding paths. | Use the same text lane and vertical alignment in both states. | Keep the title baseline stable when editing starts or ends. |
| The source specifies a 34-unit row. | Calibrate a shared row height against matched Notion and Moss captures. | The current Notion single-line sample is about 38 logical px; wrapped rows can grow. |
| Tab uses general focus traversal. | Define table cell traversal and verify the actual focus result. | The recorded Notion action selects the next property cell. |

Add a UI regression check that compares row and adjacent grid boundaries, title text baseline, and focus location before editing, in a blank new row, after Return, and after Tab. Check that failed saves keep focus. Do not infer exact colour from the profiled JPEG reference.

## Implementation after this audit

The findings above describe the earlier source state. The shared checkout now keeps one stable 38-unit cell wrapper for resting and editing states, with an internal focus outline and one text lane. Table Tab traversal selects the next loaded cell; Return opens its typed picker. Date and Status pickers and a direct Checkbox control use the shared save path. Failed saves retain the editor and block navigation.

A Rune regression checks the grid and text-lane geometry. The isolated native review also created and typed a new Table row without the displaced title in the user report. A later review showed the saved row, Status pills, Full date labels, and a relation title. That review used a synthetic workspace at 1440 × 900 logical points, so it is not a matched pixel comparison with the 1800 × 1130 Notion study.

Shift-Tab, end-of-row navigation, wrapped cells, filtered-creation reconciliation and exact source focus return remain open. The next compact-toolbar pass also found that individual tabs stretched; their growth is removed while the available toolbar lane can grow. Its existing narrow-window navigation test passes.
