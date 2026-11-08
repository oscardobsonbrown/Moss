# Database interaction work

The 2026-11-08 [video review](../research/notion/DATABASE-VIDEO-REVIEW-2026-11-08.md) covers all ten retained database layouts through eleven scene analyses. The current-source audit below was done in parallel. This is the basis for the next implementation work, not a claim of complete behavior. Keep the agreed icons, menu sizes, input styles and motion rules.

## Shared behavior

A view changes how the same records are displayed. Property edits must remain consistent across views. Layout, visible properties, filters, sorts, grouping, opening mode and source choices must be saved against stable view and property IDs. A missing or changed property needs a clear repair path rather than a silent switch to a different property.

Setup must include the information a layout needs: a grouping property for Board, a Date property for Calendar and Timeline, and a Place property for Map. Offer a suitable existing property or a working creation path. A renderer that happens to find the first property does not provide that setup flow. Linked-source selection is also demonstrated in the videos; the current local data model needs a separate implementation check for linked views.

Creation needs to account for its context. A column supplies a group value, a day supplies a date, and a filtered view can supply unambiguous property constraints. Define conflicting and ambiguous constraints explicitly. Do not change values silently to satisfy an OR rule. After create or edit, reconcile the visible query without losing the draft or inventing a separate section of new records.

## Current code and required work

`V`, `D`, `C` and `U` use the evidence meanings in the video review. Proposed failure, keyboard and recovery rules below are Moss acceptance requirements; they are not claimed video observations.

| Before: current Moss | After: required behavior | Why / evidence |
| --- | --- | --- |
| Shared create selects name and one of six layouts; new views start with an empty query and config. | Add layout-specific property/source setup and retain stable settings during duplicate, reorder and layout changes. | V01–V08 show setup and reusable views. Current lifecycle saves and conflict checks should be retained. |
| Board groups and group-context New work; existing cards only open. No card/group drag handlers exist. | Move cards between groups by changing the typed grouping value; add group ordering, visibility, collapse and subgroup controls. Add Board card layout settings. | V01 and V03. The Board comparison in V09 also needs a focused direct-property-edit check. |
| Calendar uses the first Date property, one loaded record page and only range start dates. Days use double-click creation. | Save the selected Date source; query the visible date interval; show ranges; add a day plus, Month/Week controls and direct scheduling/movement. | V01, V04 and [Calendar Help](https://www.notion.com/help/calendars). Counts and No date results must cover the matching database, not just the loaded slice. |
| Timeline has a fixed 30-day axis, a new-range footer drag and clickable existing bars. | Add source selection, Today, scales, optional table, moving/resizing existing bars, grouping, sub-items and connected dependencies. | V01 and V05. Existing dependency engine code is not yet an app interaction. |
| Table has typed cell edits and same-row Tab traversal, with fixed 280/200 pt column widths. | Add column resize/reorder, row selection/bulk actions and row movement. Define keyboard selection and boundary behavior from further evidence. | V02 and [Table Help](https://www.notion.com/help/tables). Videos do not establish exact keyboard row-edge rules. |
| Gallery has responsive sizes, content/none previews, visibility and opening mode; no grouped layout or image-source preview. | Add grouped galleries and complete preview-source, image-fit and reposition controls. Preserve page-opening behavior. | V06; [Gallery Help](https://www.notion.com/help/galleries). Direct editing of every displayed card property remains U. |
| List displays properties and opens items; it has no grouped layout or subgroup controls. | Add property ordering, groups and sub-items while preserving compact rows and page opening. | V07 and [List Help](https://www.notion.com/help/lists). Opening a title is not a missing inline editor. |
| Chart, Feed, Map and Dashboard show unavailable text. | Build real setup and data interactions for each retained layout. | V08–V11 and current Help. These require complete feature paths, not only layout polish. |
| Filter controls append direct AND conditions; sort priorities cannot be reordered. | Show existing rules, support nested AND/OR groups and editable sort priorities, and preserve view isolation. | Shared workflow in V01, V02, V04 and V06; current core already accepts structured queries. |

## Implementation order

1. **Correct the shared data and setup paths.** Add per-view property IDs, missing-property repair and filter-aware creation. Add interval and undated queries so Calendar/Timeline do not omit off-page records. Keep reads bounded; do not solve coverage by loading the whole database into the UI.
2. **Complete one Board workflow.** Start with a task database, create a Board, choose Status, create a card in a column, drag it to another column and check its value in Table. Then add group visibility/order and subgroup changes. This is a clear cross-view test from V01.
3. **Complete date workflows.** Use one database with Publish date and Deadline views. Add from a day, schedule an undated record, move a dated record, change a range and reopen it. Then add Timeline scale/table controls and dependencies.
4. **Complete Table manipulation and shared rule editing.** Resize/reorder columns, select rows, apply a bulk edit, manage filters/sort priorities and retain selection across view changes. Confirm keyboard rules before implementing assumed shortcuts.
5. **Complete Gallery and List organization.** Group, order visible properties, set previews and opening mode, and use sub-items where supported. Keep view-specific settings independent.
6. **Build the four remaining layouts.** Chart needs axes, aggregation, series and drilldown. Feed needs expandable body previews and local comments. Map needs a Place source, pin/page actions and non-blocking map/network states. Dashboard needs saved widget sources/configuration, edit/view modes, resizing and global filters. Keep all four in v1 scope.

## Acceptance paths for each implementation

- **Setup:** create from a blank database and from an existing source; change layout; select a second compatible property; delete or change that property elsewhere and recover safely.
- **Board:** move into and out of the empty-value group; preserve Boolean/number/select types; change both group values for a subgroup drop; cancel without a write; check active filters and counts; reload and compare Table.
- **Calendar:** use the chosen Date property rather than the first; include a matching record beyond the first loaded page; display a range across days and month edges; schedule from No date; move dates without losing time or metadata; exercise Month/Week, Today and keyboard creation.
- **Timeline:** move a whole range while retaining its length; resize either end with a valid range; change scale without changing dates; expose offscreen items; open the optional table; add/remove a dependency and report an invalid schedule. Test shift policies separately before assuming them.
- **Table:** resize within bounds, reorder with hidden/frozen columns, select multiple rows and verify atomic bulk updates. Check traversal, editing, pickers and scroll retention after the change.
- **Gallery/List:** change grouping and visibility without changing source records; set property order; keep card/page opening distinct from property actions; check preview changes and narrow widths.
- **Chart:** calculate against the full matching query, including nulls and zero values; open a category drilldown and its record; change a legend/filter without corrupting the source query.
- **Feed:** expand and collapse body content, open the page, adjust visible properties and use retained local comments. Empty-property and byline behavior need explicit tests.
- **Map:** choose a Place property, assign a place and locate its pin; pan/zoom and open a record; keep saved local work usable while search or tiles fail. Provider and offline behavior remain design gates.
- **Dashboard:** add a widget from a saved view, alter the widget’s query/layout, duplicate it, resize/reorder it and exit Edit mode. A matching global filter must affect the intended widgets without rewriting their source views.
- **All mutations:** retain stable record IDs; preview before release; use guarded writes; cancel without changing saved values; retain a useful draft on failure; reject a stale same-field edit; preserve unrelated edits. Provide an appropriate keyboard path and reversible action. These are implementation safety requirements, not claims from the videos.
- **All views:** reopen offline, preserve settings, check both themes and minimum width, and measure actual input/frame behavior on representative large data. Do not use a video’s apparently smooth movement as performance evidence.

## Source entry points

The audit found these paths in the shared checkout:

- [Shared view controller](../crates/moss-app/src/view_management.rs), [view UI](../crates/moss-app/src/ui/view_management.rs), [core view creation](../crates/moss-core/src/view_management.rs).
- [Record creation and cell traversal](../crates/moss-app/src/records.rs): `create_values`, `create_item`, `open_record` and table-key handling. Context values currently bypass active filters/limits while a new item is held.
- [Board/List/Calendar](../crates/moss-app/src/ui/database.rs): `board`, `list`, `calendar`, `date_property`, `undated_panel`.
- [Timeline](../crates/moss-app/src/ui/timeline.rs), [Gallery](../crates/moss-app/src/ui/gallery.rs), [Table](../crates/moss-app/src/ui/table.rs).
- [Property query edits](../crates/moss-app/src/database_properties.rs) and the standalone [dependency engine](../crates/moss-engine/src/timeline.rs).

Existing tests cover saved view lifecycle/conflicts, typed group-context creation, Gallery settings and previews, Table edits/calculations, and creation of Timeline ranges. They do not establish the missing direct manipulation paths. This study did not change app code or rerun those tests.

## Reference checks still needed

Confirm native drag thresholds, drop feedback, escape/focus behavior, Undo boundaries, multi-selection, manual order with active sorts, keyboard boundaries and date-range edge rules. Confirm the Dashboard video’s timing/completeness conflict. Form and the separate Notion Calendar app remain excluded; cloud plan limits and collaborative counters are not local product requirements.
