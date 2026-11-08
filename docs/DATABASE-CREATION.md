# Database creation

Date: 2026-11-07. Native Notion was checked with task-created sample data. Raw screenshots stay in the ignored research folder.

## Create paths

| Control | Moss behavior |
| --- | --- |
| Table footer New page | Create a blank saved row and focus its title cell. Enter ends editing. |
| List footer New page | Create a blank saved row and focus its title. Enter ends editing. |
| Board group footer New page | Create an inline card with the selected group value. Enter saves and starts another card in that group. |
| Gallery New page | Open a saved record page in a centre peek. |
| Calendar blank day, double-click | Open a saved record page with that date in a centre peek. A single click does not create an item. |
| Timeline table footer New page | Create an undated inline item. Enter ends editing. |
| Timeline empty track, drag | Show the selected range, then create a dated item with its inline title on the bar. Both drag directions work. |
| Toolbar New | Open a saved record page in the view's open mode. |

The observed configurations use Side peek for Table, Board, List and Timeline, and Centre peek for Gallery and Calendar. Moss uses these initial configurations. A view config can override `open_mode` with `side` or `center`; a settings control for this is still required. These are observed configurations, not a claim that all Notion views have the same defaults.

A record page has a live title, properties and editable blocks. Enter from the title starts the page body. Open as page uses the same saved page in a tab. Record pages do not appear as separate Private sidebar pages. Title changes in a row, peek or full page update the linked record and page together.

Blank titles are empty values. Untitled is a display placeholder. Creation saves the item before text input. Escape ends inline editing and keeps the item, including a blank item. There is no Save or Cancel dialog for record creation. Property validation and storage errors keep the input available.

## Checks and limits

Rune runtime tests cover Table and List creation, autosave, Enter, blank Escape, Board group inheritance and repeated entry, Gallery and toolbar page creation, title/body persistence, full-page title links and restored tabs, Calendar double-click, Timeline range drag and undated creation, and invalid-input retention. Core tests cover record-page ownership, atomic rename, Trash/restore, archive metadata and old archive checksums.

The reference study did not establish Calendar day plus, Board header plus, templates, every toolbar/view combination, or all custom open-mode settings. These paths still need reference work. Typed property pickers and complete page controls remain part of the wider implementation work. The six layouts still read the current 100-record page; this change does not establish full view parity or the performance targets.
