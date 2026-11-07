# Implementation status

Date: 2026-11-07. This is a working development build. It is not the complete v1 in the PRD.

## Working path

- Native Rune window with a quiet sidebar, page tabs, and system/light/dark appearance.
- SQLite WAL storage with automatic saves, page/block CRUD, workspace-scoped FTS search, and paged database queries.
- Bundled Geist Sans UI, Geist Mono code, and per-page Default/Serif/Mono, Small text, and Full width in the Appearance & layout submenu. These settings survive reopen and archive restore.
- Page titles, rich inline text, text/heading/list/to-do/quote/callout/code/divider blocks, block conversion, duplicate and delete.
- Page creation, nested-page ownership, Trash and restore. Expired Trash is removed when the store opens.
- Database creation with view-specific inline and page-peek paths, live titles and page bodies, linked record pages, simple-property editing, title search, and 100-record pages. See [Database creation](DATABASE-CREATION.md).
- Table, board, calendar, timeline, gallery and list layouts use saved records. These are initial layouts; full controls and behavior remain to be implemented.
- Existing local workspaces have separate windows. Closing a window keeps the process alive; Quit exits. The last workspace and its tabs are restored at launch.
- Formula evaluation, rollups, in-memory query helpers and date-dependency calculations have tests. Formula properties and relation/rollup editors are not connected to the app yet.
- Versioned JSON archive core with corruption checks, new IDs on import, separate-workspace restore, relations and Trash round trips. The app has archive export. This early archive version has a 64 MiB limit and rejects attachments and external relations.
- Five optional demo workspaces. Set `ROTION_DEMO=1` in a new `ROTION_DATA_DIR` to create them. Normal first launch creates one personal workspace.

## Checks

The workspace test suite covers create/edit/format/search/reopen through the Rune runtime, exact rich-text serialization, selection splitting, tab close behavior, empty and structured cells, persistence, filtered paging, workspace boundaries, Trash, archive round trips, invalid archives, formulas, rollups and date dependencies.

The storage probe has run with 100,000 pages and 1,000,000 records. Read [the measurements and failures](PERFORMANCE-RESULTS.md). It labels warm-cache storage results separately from UI and uncached disk requirements. The 38 tests pass. The earlier theme capture did not refresh. Native captures now show live font changes in the Appearance & layout menu; dark and light renderer snapshots also cover the layout. A local app bundle has been opened and used to create a database and record through the native UI. Native checks also confirm Table inline creation and Gallery centre-peek creation with editable saved page content.

## Required work before v1

The PRD remains the scope. In particular:

- Complete the editor: nested block behavior, columns, cross-block selection, structural undo, drag/reorder, links, mentions, backlinks, synced blocks, media and file previews.
- Match measured Notion geometry, typography, control states and behavior. Current icons and some controls are temporary. Continuous reference recordings and paid-plan state evidence remain missing.
- Complete typed properties, live formulas, relations, rollups, linked databases, filters, sorts, groups, templates and dependency editing. Some engine/store filter semantics still differ.
- Complete Chart, Feed, Map and Dashboard, and full settings and interactions in the six initial layouts. Current views operate on the current record page, so group/calendar totals are not global totals.
- Add creation and import of workspaces with new windows at runtime. Rune currently registers workspace windows before the event loop starts. Resolve this framework boundary before exposing those actions.
- Add local people, comments, buttons, automations, recurring templates, reminders and optional Start at login. The resident process alone does not implement scheduled work.
- Complete attachments and lossless archive support at the agreed scale. Add Markdown/CSV/HTML import and Markdown/CSV/HTML/PDF export with loss reports.
- Complete per-tab scroll/selection restoration, navigation history, accessibility, keyboard/IME, error/recovery and disk-full checks. Keep unsaved edits available after write failures.
- Measure full visible content-open time, uncached reads, memory and cold start on this Mac. The 8.8 ms, 20 ms and 300 ms requirements are not yet verified. JSON property queries still scan records; they need indexed projections for large data.
- Sign, notarize and test a public release. Nothing has been published.
