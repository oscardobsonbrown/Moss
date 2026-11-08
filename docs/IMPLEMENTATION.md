# Implementation status

Date: 2026-11-07. This is a working development build. It is not the complete v1 in the PRD.

## Working path

- Native Rune window with a quiet sidebar, page tabs, and system/light/dark appearance.
- SQLite WAL storage with automatic saves, page/block CRUD, workspace-scoped FTS search, and paged database queries.
- Bundled Geist Sans UI, Geist Mono code, and per-page Default/Serif/Mono, Small text, and Full width in the Appearance & layout submenu. These settings survive reopen and archive restore.
- Page titles, rich inline text, text/heading/list/to-do/quote/callout/code/divider blocks, block conversion, duplicate and delete.
- Page creation, nested-page ownership, Trash and restore. Expired Trash is removed when the store opens.
- Database creation with view-specific inline and page-peek paths, live titles and page bodies, linked record pages, simple-property editing, title search, and paged records (Gallery defaults to 50). See [Database creation](DATABASE-CREATION.md).
- Table, Board, Calendar, Timeline, Gallery and List layouts use saved records. Table cells keep a shared grid during edits and support cell traversal and direct Checkbox toggles. Date, Status and Select use typed pickers. Gallery size, preview and load limit persist, and its first-block previews use one bounded query. Calendar Today and Calendar/Timeline No date search are connected. Timeline uses one axis for date labels, clipped ranges, weekend bands and a Today line. Its geometry is provisional. Full controls and behavior remain to be implemented.
- Existing local workspaces have separate windows. Closing a window keeps the process alive; Quit exits. The last workspace and its tabs are restored at launch.
- Formula, Relation and Rollup editors are connected to transactional materialized calculations. Local source edits update dependent rows, with cycle guards and explicit calculation errors. This is a tested subset of Notion formulas, not complete syntax or date-time parity.
- Versioned JSON archive core with corruption checks, new IDs on import, separate-workspace restore, relations and Trash round trips. The app has archive export. This early archive version has a 64 MiB limit and rejects attachments and external relations.
- Five optional demo workspaces. Set `MOSS_DEMO=1` in a new `MOSS_DATA_DIR` to create them. Normal first launch creates one personal workspace.

## Checks

Local hot reload is available through `scripts/dev.sh`. It uses the pinned Rune launcher, Dioxus CLI 0.7.10, two compiler jobs and separate data in `target/dev-data`. UI and rich-text function bodies can patch without a restart. Other files trigger rebuilds; incompatible declarations in patch files require a manual restart. Normal builds exclude the hot reload runtime.

On 2026-11-07, the hot feature compiled and a persistent headless renderer applied a temporary sidebar label change and its restore. Both kept the same process ID, state address, scroll values and open Settings dialog. The patch compiler reported 2.192 s and 3.931 s. The first full hot build took about 106 s. These are development checks, not product performance results. No native-window hot reload check was made. The temporary source edit was restored and the test server was stopped.

The workspace test suite covers create/edit/format/search/reopen through the Rune runtime, exact rich-text serialization, selection splitting, tab close behavior, empty and structured cells, persistence, filtered paging, workspace boundaries, Trash, archive round trips, invalid archives, formulas, rollups and date dependencies.

The storage probe has run with 100,000 pages and 1,000,000 records. Read [the measurements and failures](PERFORMANCE-RESULTS.md). It labels warm-cache storage results separately from UI and uncached disk requirements. The latest combined run passed 207 tests (141 app, 53 core and 13 engine), including the minimum-window Date, popup hit-test, Timeline axis and motion checks. The earlier theme capture did not refresh. Native captures now show live font changes in the Appearance & layout menu; dark and light renderer snapshots also cover the layout. A local app bundle has been opened and used to create a database and record through the native UI. Native checks also confirm Table inline creation and Gallery centre-peek creation with editable saved page content.

## Required work before v1

The PRD remains the scope. In particular:

- Complete the editor: nested block behavior, columns, cross-block selection, structural undo, drag/reorder, links, mentions, backlinks, synced blocks, media and file previews.
- Match measured Notion geometry, typography, control states and behavior. Current icons and some controls are temporary. Continuous reference recordings and paid-plan state evidence remain missing.
- Complete the remaining typed properties, full Formula/Relation/Rollup parity, linked databases, advanced/nested filter editing, grouping details, templates and dependency editing. The current header and advanced-property controls cover a working subset; some filter semantics still differ.
- Complete Chart, Feed, Map and Dashboard, and full settings and interactions in the six initial layouts. Current views operate on the current record page, so group/calendar totals are not global totals.
- Add creation and import of workspaces with new windows at runtime. Rune currently registers workspace windows before the event loop starts. Resolve this framework boundary before exposing those actions.
- Add local people, comments, buttons, automations, recurring templates, reminders and optional Start at login. The resident process alone does not implement scheduled work.
- Complete attachments and lossless archive support at the agreed scale. Add Markdown/CSV/HTML import and Markdown/CSV/HTML/PDF export with loss reports.
- Complete per-tab scroll/selection restoration, navigation history, accessibility, keyboard/IME, error/recovery and disk-full checks. Keep unsaved edits available after write failures. App navigation has save guards; native termination still needs an all-window veto path (see NATIVE-CLOSE-GAP.md).
- Measure full visible content-open time, uncached reads, memory and cold start on this Mac. The 8.8 ms, 20 ms and 300 ms requirements are not yet verified. JSON property queries still scan records; they need indexed projections for large data.
- Sign, notarize and test a public release. Nothing has been published.

## UI review changes

| Before | After | Why |
| --- | --- | --- |
| Header menus only renamed and deleted properties. | Compact menus provide typed filters, sorting, grouping, page icons, calculations, freezing, wrapping and insertion on either side. | Implement the retained controls from the supplied Notion reference. |
| Formula and Rollup cells displayed stored values without recalculation. | Materialized calculations propagate through local property dependencies and Relations, with atomic validation and visible errors. | Keep computed results correct when source records change. |
| Relation titles depended on an open target tab. | Bounded batch caches supply visible related titles without storage reads during rendering. | Keep closed target databases readable in Relation cells. |
| Whole-database summaries could block the UI. | A read-only background worker coalesces requests and rejects stale results; shutdown interrupts and joins the worker. | Keep full-filter aggregation off the UI thread. |
| The stopped Hedvig change had only a regular face. | Optional local Times Newer Roman provides real regular, bold, italic and bold italic faces, with a bundled Geist Serif fallback. | Supply the requested serif styles without redistributing the optional font. |
| Move to Trash used ordinary menu text. | Its text is red in the page action and sidebar context menus. | Mark a destructive action clearly. |
| Sidebar pages had no secondary-click menu. | A pointer-anchored menu offers working Open, Open in new tab, Rename, New subpage and Trash actions for the clicked page. | Act on inactive pages without losing the current selection. |
| The first page context menu used 32-point rows and a 266-point width. | Compact 26-point rows, 13-point text, 16-point icons and a 232-point width match the sidebar density. | Keep secondary actions smaller than the navigation rows. |
| Rune painted each caret continuously. | Shared inputs use a 500 ms blink timer with editing/selection reset and steady IME feedback. | Provide standard caret feedback without a continuous frame loop. |
| Table columns could not be created, renamed or deleted. | Header plus and property popups provide typed creation, rename and confirmed deletion. Name remains required. | Complete the requested database column controls. |
| Property popup labels and icons used different insets and natural widths. | A shared 10-point inset, fixed 20-point icon/check boxes and 32-point rows align the controls. | Give each row a stable text lane and balanced spacing. |
| The earlier width change made sidebar New page compact instead of the Table footer. | Sidebar New page again fills the same width as Settings and Trash. Table footer New page fits its icon and text. | Apply the user's corrected scope and keep sidebar rows consistent. |
| Every record used a lined page icon, including metadata-only pages. | The default icon has no lines for an empty body and two lines for saved body content. Custom icons stay in place across Table, Board, Calendar, Timeline, Gallery and List. | Make the page body state clear without opening the record. |
| A million-record database still exposed only a 100- or 999-record page. | Plain fixed-height Tables use one logical scroll range, bounded ordinal reads, and leading/trailing space around mounted rows. | Keep the full dataset reachable without allocating a million row objects or layout keys. |
| The sticky Table header could paint over open dropdowns and receive their clicks. | Root overlays use the later raised layer for both drawing and pointer input, above sticky headers and frozen columns. | Keep menus and dialogs above the content they control. |
| The toolbar always showed a fixed number of views and a More count. | Shaped Geist text and the stable toolbar width determine which tabs fit. Only real overflow has a More count; view settings and creation have separate controls. | Keep all available space useful and make the two actions distinct. |
| The view chooser used names without type icons. | All saved view kinds have a shared SVG icon; the active view has a checkmark. | Make layouts clear in the tabs, chooser and settings. |
| View changes had no lifecycle manager. | Guarded controls create, rename, duplicate, move, delete and change layouts; names autosave and failed drafts remain available. | Make saved views usable without schema edits or dead actions. |
| The shell left its sidebar titlebar area empty. | Sidebar controls share the titlebar with back/forward navigation and the existing thin tabs. Search remains directly below. | Follow the supplied Notion structure while preserving the approved tab surfaces. |
| Serif mode used the prior local Times Newer Roman installation. | It now uses four local Sentient faces, with Geist Serif fallback. | Apply the latest type direction while keeping fonts outside the public package. |
| View chooser, Layout, settings and property rows used different heights, insets and icon boxes. Some checks were part of the label text. | One shared menu row uses 32-point height, 10-point side insets, 20-point icon and trailing boxes, 16-point SVGs and an 8-point label gap. Checks and chevrons have their own column. | Keep each menu pane aligned and prevent long labels from displacing controls. |
| Workspace selection opened a centered dialog. | A 300-point popup opens above the sidebar control, with a workspace header, Settings, and checked local workspace rows. | Follow the supplied Notion layout within the local-only scope. |
| Sidebar hover backgrounds could merge or use different widths. | Each page has one full-width, 26-point rounded highlight in its 30-point row, with instant hover changes. | Keep selected and adjacent hovered pages distinct. |
| The Table editor replaced its cell wrapper. | Rest and edit share the grid and text lane. | Keep new-row titles and adjacent cells aligned. |
| All database view buttons crowded the toolbar. | Compact tabs and a searchable view list keep every saved view reachable. | Keep controls usable at the minimum window size. Popup geometry is still under review. |
| Scalar text stood in for Date and Status controls. | Typed anchored pickers preserve failed drafts and structured values. | Provide direct entry and safe save behavior. Ranges and optional times are editable; reminder and format controls, mixed time precision, and calendar end-date selection remain open. |
| Gallery used fixed card rows and blank previews. | Cards wrap at saved sizes and read the first block in one query. | Keep the selected layout and saved content coherent. |
| A search or New page action could run before a failed draft was saved. | Navigation and creation check the save first. | Retain the draft and avoid orphan page creation. |
| The first tab started above the sidebar. | The strip starts 8 points after the sidebar split and follows its width. | Align navigation with the editor pane, as requested. |
| Tabs used a fixed 174 × 36-point block and an underline. | Tabs are 28 points high, size to their title up to 240 points, and use a subtle rounded selected background. Overflow scrolls within the strip. | Follow the user's thinner Paper app reference. |
| Light tabs used a gray selected surface and showed every close control at rest. | The active light tab has a white surface, fine border and soft shadow, with its close control visible; inactive close controls appear on hover. Dark styling and stored page icons are preserved. | Follow the supplied Paper reference while keeping the compact tab geometry and existing pointer and keyboard motion. |
| The editor pane met the sidebar at a square corner. | The pane and its children use a 10-point top-left corner. | Keep the workspace visually separate from the shell. |
| The sidebar stayed at 270 points wide. | A six-point divider supports captured pointer drags and keyboard resizing. The width is saved per workspace on release. | Let the user set the available navigation space without disk writes during drag frames. |
| Narrow content could hide view tabs and crowd controls. | View tabs and toolbar controls wrap. Page margins, Gallery rows and side peeks use the available width. | Keep controls usable as the sidebar or window changes size. |
| New page opened a name/type dialog. | New page opens a saved empty page with an editable title and inline Page and Database buttons. Dashboard stays hidden. | Use the requested page-first creation flow; dashboarding is not implemented yet. |
| Home, footer New page and the Private plus used several sidebar locations. | New page sits below Search. Home and the other two creation controls are removed. | Keep one clear page creation action. |
| Sidebar icons used different left insets. | Root page, navigation and workspace icons align with Private; child disclosures use the right edge. | Keep the requested icon column consistent. |
| Table Open used a full-height generic button. | A 70 × 24-point control uses a peek icon, OPEN label, rounded border and the tab's selected fill. | Match the supplied reference size within the table row. |
| Database view controls had small corner radii and tight sides. | View controls use full pill corners and 14-point horizontal padding. | Apply the user's requested shape. |
| Input text and carets used different offsets across controls. | All editable controls use shared input helpers; boxed fields use a 20-point line with symmetric padding in 32 points. | Keep text, placeholder and caret aligned in both themes. |
| New was a separate blue action. | A blue rounded split button creates items or opens Empty item and page-opening options. | Keep the toolbar controls consistent. |
| Long titles could crop and used separate input styles. | Titles wrap to the available width, retain the page font, and move focus to the body on Enter. | Keep narrow pages editable. |
| Placeholders competed with saved text. | Ordinary inputs and large titles use separate softer hint colors. | Keep typed text clear and empty fields subtle. |
| Table headers used plain text. | Each property has a type icon, a subtle header surface and a stronger bottom rule. | Make column types and the header row clear. |
| The block handle could disappear on the path from the text. | A shared 30-point hover gutter keeps the 20 × 24-point handle reachable; focus and an open menu also keep it visible. Pressing it preserves selection. | Allow reliable access to block actions. |

The earlier toolbar changes passed all 93 app tests, including menu creation, saved opening modes, Full page tabs, stale-view rejection, draft guards, and pointer/keyboard motion. Dark and light renderer previews cover the compact Open control and the menu at 850 × 600. Native capture remains unavailable.

Opening mode is saved through the atomic view-config API and preserves other settings. Full page mode opens the linked record page in a tab. Failed settings leave the menu open with an error; failed drafts block full-page navigation and creation. The current menu scope was confirmed by the user; database templates remain future v1 work.

The earlier inline creation flow passed the full 124-test workspace suite. Dark and light renderer previews confirm the page choices, sidebar action placement and icon column at full and minimum window sizes. Native capture of the development executable remains unavailable.

Database selection converts the same empty document page in an Immediate transaction, with the database definition and search update. It preserves the latest page title, parent and settings. Insert failure, existing content and Trash leave the original document unchanged. Page selection reads any content added since the chooser opened and does not overwrite it. Empty documents can show the choices again until content is saved. Subpages remain available from Actions → New subpage.

Sidebar widths default to 270 points and are bounded between 200 and 480 points. The visible width reserves 400 points for content and more space when a side peek is open. Window constraints do not replace the saved preferred width. Cancellation restores the pre-drag width. Tests cover repeated drag moves with the handle moving, save on release, cancellation, reopen, old preferences, independent workspace widths, keyboard limits, all six implemented database layouts, and side-peek open/close at the minimum window size.

The complete app suite passed 87 tests after the sidebar changes. The core and engine were unchanged. Renderer checks cover a 480-point sidebar in a wide window and its constrained width at 850 × 600. Native pointer dragging remains unverified because the computer-use tool cannot attach to the development executable.

The tab and corner change passed all 13 existing motion tests. Renderer images were checked in light and dark themes, with two tabs and at the 850 × 600 minimum window size. The tree confirms the sidebar/pane boundary and tab inset. The live development server accepted the patch; the computer-use tool could not attach to its unbundled executable for a native screenshot.

Paper now has 48 editable component boards and 22 desktop screens. It remains a reference study with stated gaps. The continuous discovery-free hour has not been met.

Tested pointer menus, pickers and peeks fade. Keyboard openings and database view changes stay instant. Tab content appears before its pointer indicator finishes. These are Rune runtime checks; continuous native recordings remain unavailable. See MOTION-AUDIT.md for timings and gaps.

The sidebar now reads bounded sibling groups, with disclosure controls, saved expansion and safe stale-cursor recovery. Empty search suggestions use a separate bounded workspace query. Full Document and Record pages show bounded child links, with save guards, refresh and retry paths. These paging and child-link layouts remain provisional until native comparison is reliable.

Property autosaves compare and update one value in an Immediate transaction. They preserve other saved properties, reject same-field conflicts and retain failed drafts. View configuration edits merge the current saved config under the same write lock and reject a changed view layout. Database creation commits Page, Database and search entry together. Failed scalar edits offer Retry and explicit reload; failed picker discard also reloads the saved record before closing.

The input and block-handle pass passed all 136 workspace tests (102 app, 27 core and 7 engine). The block tests cover pointer travel from focused and unfocused text through the gutter, preserved text selection, menu focus, dismissal and reopening. Shared input tests use the bundled fonts at 1× and 2×. Dark and light renderer previews cover input states, wrapping and the revised toolbar. Native pointer and IME checks remain unverified because capture cannot attach to the development executable.

The workspace popup and sidebar hover pass passed all 140 workspace tests (106 app, 27 core and 7 engine). Popup checks cover light/dark themes, narrow/wide sidebars, screen bounds, no scrim, active selection, Settings, unavailable workspace errors, Escape, outside dismissal, focus return, pointer fade and instant Space activation. The sidebar check covers nested row geometry, immediate hover transfer and selection changes. Renderer previews show the popup in both themes at 1× and 2×. The dev server runs the updated source; native capture remains unavailable.

The database column pass passed the full 151-test workspace suite (113 app, 31 core and 7 engine). Core tests cover persistence, atomic rollback, stale-schema guards, Title protection and cleanup of property values, search and known view settings. App checks cover failed drafts, Select option validation, inactive-tab refresh, header creation/rename/delete, cancellation, typed cell persistence, focus return and narrow popup bounds. The three UI integration tests passed again after alignment changes. Renderer previews cover dark creation and light deletion. Native mouse checks remain unverified.

The sidebar context menu and caret pass passed all 162 workspace tests (122 app, 33 core and 7 engine). Context tests cover secondary-click selection, menu bounds, dismissal/focus, inactive-page rename, new tabs, subpage parents, descendant Trash and failed draft guards. Core rename tests cover atomic title conflicts, current metadata, search and rollback. The caret test checks actual rendered caret rectangles for selected and collapsed text, the 500 ms phase, selection reset, IME composition and window blur. Synthetic dark/light renderer previews cover the menus. Native pointer and IME checks remain unverified.

The Table/calculation pass implements persistent header actions with guarded view edits, relative column insertion, per-tab search state, frozen columns and wrapped editing. Summaries cover all matching records, independent of loaded pages. They run in a read-only worker and can show a pending state; the first-viewport timing contract is not established by that design. A stale result cannot replace a newer request. Worker shutdown is joined before State teardown.

Formula configs store `expression` and generated stable `_bindings`; Relation configs select `target_database_id`; Rollup configs select `relation_property_id`, `target_property_id` and an operation. Source edits, linked record rename, Trash/restore, schema changes and archive import participate in tested recalculation paths. The engine supports basic arithmetic, booleans, conditions, text/number/date functions, regex and pure-list operations including `map`/`filter` with `current`/`index`. Relation-page traversal such as `current.prop(...)`, clock-driven `now`/`today`, full date-time semantics and all Notion functions remain open. Unsupported expressions and invalid calculations return explicit errors.

Relation search has bounded results but its substring plan is not indexed. Visible title caches scan at most 5,000 relation entries and resolve at most 1,000 distinct IDs per tab; missing, deleted and excess IDs retain a count fallback. Refresh detects this Store connection's writes and coalesces visible updates; cross-connection invalidation remains limited. Active input and peek drafts are preserved, and new items remain visible through the existing creation flow. These bounds and passing tests do not verify the 8.8 ms, 20 ms or 300 ms targets.

Times Newer Roman was installed locally in four unmodified OTF styles. The renderer loads them once at launch and tests verify actual face selection for all four styles. They are optional local files, not included in Moss's MIT source or packaged resources. Saved Serif settings select the installed family without a migration. The upstream license is kept with the local installation; public font distribution is not part of this change.

Final combined verification for this batch passed 207 tests (141 app, 53 core and 13 engine), formatting and whitespace checks, and the debug build. Renderer checks cover the header popup in dark and light themes, narrow-window scrolling, and a real Serif page using locally installed Times Newer Roman. The dev server rebuilt successfully with the current source. Native pixel comparison, full Notion formula compatibility and the specified scale/timing limits remain unverified.

The next combined pass on 8 November passed 222 tests (151 app, 58 core and 13 engine; one scratch performance benchmark is ignored in the normal suite). Checks cover body-aware record icons across all six implemented database views, saved-body changes and clearing, custom icons, compact Table creation, full-width sidebar creation, light and dark tabs, variable-height Table virtualization with wrapped/frozen/grouped rows and summaries, and indexed paging with duplicate titles and per-tab search. Formatting and whitespace checks pass. Light and dark renderer previews cover wide and minimum-size windows. Native scroll measurements and their limits are recorded separately in PERFORMANCE-RESULTS.md; these functional tests do not prove the timing targets.

The full-scroll integration pass then passed 237 tests (160 app, 64 core and 13 engine; two performance probes are excluded from the normal suite). Plain, fixed-height, unfiltered ID-order Tables use a background ordinal window with at most 256 loaded records after preparation, plus at most 100 pinned new records. The initial real-data page keeps its existing load bound until that window is ready. An index build or rebalance runs off the UI thread; existing real content stays available during preparation. Request generations reject stale results, obsolete queued requests are pruned, and worker responses wake the native app. Shutdown interrupts SQLite work and joins the worker. A versioned trigger migration fixes rank updates during record UPSERT without replacing user data.

Filtered, sorted, grouped, searched and wrapped Tables keep bounded pagination. They are not covered by the full-million-row scroll path. Cell and picker drafts hold a scroll that would evict their cached row until save or discard; open record peeks retain their own drafts while the Table loads another window. Newly created records join the grid without a separate heading or gap. The displayed count includes successful local creations while a draft holds a pending refresh; fresh snapshots clear that delta without double-counting. Temporary retention outside a cached ordinal window does not establish manual row ordering. The first-time rank build, uncached reads, native presentation and remaining timing failures are reported separately in PERFORMANCE-RESULTS.md.

The final app regression pass covers 162 passing tests after bottom anchoring, fixed row clipping, sticky headers, exact record totals and overlay stacking. Together with the unchanged 64 core and 13 engine checks, 239 tests pass; two performance probes run separately. The overlap regression checks Views and New menus over the sticky header and a frozen column in both themes. A light 2× renderer preview confirms that the menu paints above the header. The debug build, formatting and whitespace checks pass. Real-fixture release probes reach and edit the final record in the million-row list, but native 120 Hz pacing remains unmet and continues as a separate performance investigation.

The 8 November view-management and shell pass passes 262 tests (181 app, 68 core and 13 engine; two performance probes excluded). It includes responsive, icon-led saved-view tabs; guarded lifecycle operations; saved active view IDs; per-view property visibility; filters and sorts in all six implemented layouts; and typed Board grouping. Exact Notion parity, deep view links, drag reordering and remaining advanced view settings are still open. Native computer use failed at startup; supplied screenshots and the official sources in `research/notion/VIEW-MANAGEMENT-20261108.md` are the reference.

The shared titlebar has working sidebar collapse/expand and New page controls plus guarded Back/Forward navigation before the existing tabs. Collapse and the chosen sidebar width persist per workspace. History uses a tested per-window policy; exact Notion history semantics remain unverified. The sidebar keeps the earlier New page row.

Serif pages now use four locally installed Sentient faces. Rune face-selection tests confirm the actual regular, bold, italic and bold italic bytes, while UI and code keep Geist Sans and Geist Mono. A rendered Serif page was inspected. The font files and local licence remain outside the repository; public builds retain Geist Serif fallback. Existing Times Newer Roman files are untouched.

The debug build, formatting and whitespace checks pass. Renderer previews cover the icon chooser, View settings, creation, the collapsed dark shell and seamless new Table rows. The latest dev-server rebuild succeeded. Native 120 Hz presentation remains unverified; the separate performance report records invalid foreground samples and the current display-link startup failure. Three new editable Paper boards use synthetic content and label their reference gaps.

The menu alignment pass passes 263 tests (182 app, 68 core and 13 engine; two performance probes excluded). A runtime geometry regression checks common icon, label and trailing columns across View settings, layout, opening mode, property selection, property visibility and the view chooser in both themes at 1× and 2×, including a long property name. The debug build, formatting and whitespace checks pass. Renderer previews were inspected for the light settings/property menus and dark chooser/layout menus. Native Notion comparison was not repeated.


## Sidebar personalization — 2026-11-08

Page emoji can be changed from the sidebar icon, Edit icon action, document header, record peek and page toolbar. The local Unicode Emoji 16.0 catalog has 3,781 entries, with search, category controls, bounded pages of 64 choices, session recents, Random and Remove. Atomic icon-only saves reject stale icons, invalid values, trashed pages and foreign workspaces. They retain the latest title, parent, appearance and body. Loaded sidebar, Favorites, tabs, page headers, child links and database record icons update after success. An inactive page icon opens its picker without navigating or saving unrelated drafts.

Favorites persist per workspace. Reads and visible rows are bounded to 100 IDs. Missing and trashed pages are hidden without removing their saved IDs, so restore can bring them back. A failed preference write retains the previous list. The sidebar uses the same row renderer for Favorites and Private. It caps deep indentation at the available width and reserves equal action lanes, with row actions for a compact page menu and New subpage. Existing rename, open, open in new tab, expansion and Trash controls remain connected.

| Before | After | Why |
| --- | --- | --- |
| Emoji labels had no editing action | The fixed 20 pt icon lane is a separate button with a searchable popup | Change an inactive page without changing the current page |
| Sidebar page actions required right click | Quiet row menu and subpage controls use fixed trailing lanes | Expose useful actions while the title keeps its position |
| No saved page shortcuts | A workspace-specific Favorites section uses the same page rows | Keep common pages within reach without a second visual system |
| Toolbar emoji and title shared one text string | Separate centered icon and title controls with fixed spacing | Align emoji, blank icons and text consistently |
| Child-page links always showed one generic icon | Links show the saved emoji or a fixed-size default icon | Keep page identity consistent across locations |

Reference findings and remaining limits are in [Sidebar personalization](SIDEBAR-PERSONALIZATION.md). No new native Notion state was observed: the desktop host failed before opening the app. Built-in icon catalogs, uploaded image icons, custom workspace emoji, Favorites reorder, page move/duplicate and database-view shortcuts remain open. No native frame-rate claim is made for this work.

The workspace suite passed 277 tests (193 app, 71 core, 13 engine), with two performance probes ignored. The debug app built successfully. Renderer snapshots use synthetic data in `/tmp/moss-sidebar-review-20261108/`. An existing Gallery test was made independent of random record order: it now checks that removing the small-card preview removes its 125 pt height, rather than assuming the first record has a short title.


## Phosphor interface icons — 2026-11-08

The interface uses Phosphor Regular through one shared Rune icon catalog. The selected official SVG files are bundled at revision `2b75f3ad12b420c9504ef05df8d2564a28f8500e` (core package 2.1.1), with source hashes and the full MIT notice under `crates/moss-app/assets/phosphor`. The macOS packaging script includes that notice and the Unicode data notice in the app resources. No React package, JavaScript runtime, icon font, or runtime network request is added.

The shared catalog covers sidebar and tab controls, page menus, emoji categories, editor controls, database view and property types, filter/sort/group actions, calendar and timeline navigation, selection checks and creation controls. Existing keys, hit targets, typography previews, page emoji, theme colors and fixed icon slots are retained. The default record icon uses Phosphor File for metadata-only pages and File Text for body content. A raster check verifies that File Text has exactly two interior lines.

| Before | After | Why |
| --- | --- | --- |
| Custom SVG paths were repeated across interface modules | Shared Phosphor Regular SVG constants | Keep shapes and weights consistent and maintain one source |
| Some controls used font glyphs for arrows, checks and pluses | SVG icons inside the existing control boxes | Avoid font-dependent shape and baseline differences |
| Gallery and Dashboard shared a similar grid symbol | Phosphor Images Square and Squares Four | Distinguish the two view types |
| Blank and content page icons used local paths | Phosphor File and File Text | Keep the two-line body-content rule within the same icon family |

The workspace suite passed 279 tests (195 app, 71 core, 13 engine), with two performance probes ignored. The icon raster checks cover every bundled icon at control sizes 12–24 pt and scales 1× and 2×. Existing menu and sidebar geometry tests cover both themes. The debug build, formatting, asset hashes, shell syntax and diff checks passed. Synthetic-data renderer snapshots are kept in `/tmp/moss-phosphor-review-20261108/`. Native app capture remains unavailable because the computer-use host fails to start; no native performance claim is made.


## Sidebar titlebar divider — 2026-11-08

| Before | After | Why |
| --- | --- | --- |
| Sidebar compose and page-history controls shared an unmarked gap | A 1 × 18 pt theme-colored separator sits at the sidebar boundary | Separate the two control groups without moving their existing hit targets |

The separator follows sidebar resizing and hides with the collapsed sidebar. Existing sidebar resize and shell navigation tests cover the retained geometry and actions.


## Page alignment and editor commands — 2026-11-08

The page icon, title, ordinary paragraphs, plain headings and Divider now share one leading edge. The new-page chooser uses the same narrow-window inset, and record-peek icon controls also align with their titles. The icon no longer centers in an oversized horizontal box. List markers and callout/code insets remain deliberate. The gutter has separate insert and action controls with shared hover coverage, and the page margin keeps those targets inside a narrow content pane. A blank writing-area click focuses or appends a paragraph without splitting the last block.

Slash, plus and grip use one anchored, searchable command popup for supported blocks. It has Phosphor icons, Basic blocks grouping, heading shortcut hints, keyboard selection, no-results feedback and Escape. Slash cancellation saves literal `/query` text. Code keeps literal slashes. Plus reuses an empty writing line or inserts after nonempty text. Divider creates or reuses a following editable paragraph; choosing Divider for existing text inserts below it rather than hiding the text. The grip keeps Duplicate and Delete, with focus restored to an editable block.

Enter preserves rich-text splitting and continues lists; an empty list returns to a paragraph. Typed-space shortcuts use the documented Notion heading/list/toggle/quote patterns, with the local Divider shortcut noted separately. Backspace after a Divider removes the Divider and keeps the paragraph focused. Converting a completed to-do clears its completed flag for other types. Failed command saves retain the draft and offer Retry or Keep draft and close.

| Before | After | Why |
| --- | --- | --- |
| Emoji and footer plus were centered in different box widths | Icon and document content use one edge; the standalone plus becomes a blank writing area | Remove the diagonal visual drift |
| Each block had one action handle and Slash reused a type list | Reachable plus and grip open the correct insertion or type/action mode | Separate writing, insertion and block actions |
| Type choices had no search or shortcut hints | Anchored search, fixed icon/text/shortcut lanes and keyboard selection | Find a supported block without scanning a long list |
| Empty-list Return and Divider Backspace could leave poor focus states | Type exit and Divider continuation keep a real input focused | Keep keyboard editing continuous |
| A failed command could leave its popup blocking recovery | Retry and Keep draft and close preserve the dirty document | Let the user copy or recover the draft |

The workspace suite passed 304 tests (220 app, 71 core, 13 engine), with two performance probes ignored. The new alignment tests cover 48 width/theme/scale/font/full-width combinations. Integration tests exercise actual Slash typing, search, Enter, Escape, Markdown conversion, code slashes, gutter reachability, selection, insertion, duplicate/delete focus, persistence and failed-save recovery. Debug build, formatting and diff checks pass. Synthetic renderer snapshots are in `/tmp/moss-editor-review-20261108/`.

This is a bounded editor pass, not complete Notion parity. [Editor reference and remaining gaps](EDITOR-UX.md) records structural undo, nesting, dragging, multi-block selection, the selection toolbar and cross-writer block conflict protection. The native desktop host was retried and still failed before reaching Notion.


## Collapsed sidebar reveal — 2026-11-08

The collapsed sidebar keeps a 56 pt rail with loaded page and Favorites icons. Rules of 20 × 1 pt separate the groups. Hover reveals the saved-width sidebar over the content without reflow or a preference change. A 120 ms leave grace supports movement into the panel; sidebar popups stay usable beyond its edge. Escape dismisses the temporary reveal, and keyboard pinning restores the expanded sidebar. The header compose control remains the page creation entry.

| Before | After | Why |
| --- | --- | --- |
| New page appeared below Search as well as in the titlebar | The header compose control is the creation entry | Remove the duplicate sidebar row |
| A collapsed sidebar did not retain the loaded page and Favorites icons | A 56 pt rail keeps those icons and uses group rules | Keep navigation available in the collapsed state |
| Expanding the sidebar changes the content width | Hover opens a temporary overlay without reflow or saved preference changes | Let the user navigate without moving the page |
| Pointer exit could close the panel during travel | A 120 ms grace and popup protection keep the panel available | Allow travel into the panel and its menus |

All 10 focused sidebar reveal tests pass. The workspace suite passes 314 tests (230 app, 71 core and 13 engine), with two performance probes ignored. The debug app build, formatting and diff checks pass. Synthetic light and dark rail renders and a hover preview render were checked in `/tmp/moss-sidebar-reveal-20261108/`. The ChatGPT comparison was blocked by the Sky native pipe and browser host failure. These dimensions are implementation requirements, not measured reference values. See [Sidebar reveal](SIDEBAR-REVEAL.md).

## Empty block hint — 2026-11-08

| Before | After | Why |
| --- | --- | --- |
| Every empty block showed its insertion hint at rest | The hint appears only on block hover or focus and disappears when text is entered | Reduce visual noise while keeping an empty line available for input |

Four editor alignment and interaction tests pass. The hint regression checks pointer entry and exit, focus after a click, and visible typed text after blur in both themes at 1× and 2×. Native visual checking remains blocked by the computer-use host.


## Sidebar sections — 2026-11-08

The later hover pass hides page and section actions at rest, reveals them on row hover, and uses stronger icon ink and a background on direct hover. Keyboard navigation reveals the actions. A shared visual wrapper hides only the paint; the outer button retains its hit target for fast clicks. Hover styles use Rune's paint tracks without rebuilding the view on each action hover.

| Before | After | Why |
| --- | --- | --- |
| Sidebar actions remained faintly visible at rest | Actions appear only on row hover or keyboard navigation | Reduce visual noise |
| Direct hover used the same icon ink | Direct hover strengthens the icon and button background | Identify the current action |

The full suite passes 334 tests (250 app, 71 core, 13 engine), with two performance probes ignored. Hover checks cover page, Favorite, Recent, custom-section and section-header actions in both themes at 1× and 2×, plus keyboard reachability and fast-click regressions. Synthetic row/direct-hover renders were checked in `/tmp/moss-sidebar-hover-20261108/`. The build, formatting and diff checks pass. Native visual checking remains unavailable.

The current implementation task adds per-workspace custom shortcut sections, Recent and nested Favorites. Section creation, rename, removal, collapse, drag reorder and menu reorder are in scope. Recent contains four unique actual page opens from tabs, history and peeks; startup restoration is excluded. Page actions use a 3 pt trailing offset.

| Before | After | Why |
| --- | --- | --- |
| Sidebar groups had fixed organization | Custom shortcut sections can be created, named, removed and reordered per workspace | Organize shortcuts for each workspace |
| No dedicated list of recent opens | Recent shows four distinct actual opens without counting startup restoration | Return to recently used pages |
| Favorites did not provide the requested nested navigation | Favorites supports pages and subpages | Reach saved pages within their hierarchy |
| Trailing page actions needed placement adjustment | Actions use the requested 3 pt offset | Keep row actions aligned |

The workspace suite passed 333 tests (249 app, 71 core and 13 engine), with two performance probes ignored. The app build, formatting and diff checks passed. Persistent input focus, caret tracking and stale-target refresh regressions pass. Synthetic light full-sidebar, dark section-menu and collapsed-rail renders were inspected in `/tmp/moss-sidebar-sections-20261108/`.

An index-built virtual row list combines section headers, Private, Favorites, Recent, custom shortcuts and New section in each sidebar state. This prevents overlap from a growing virtual Private list followed by sibling sections. Tests confirm that expanded Private and a following custom section do not overlap. With 1,200 shortcuts, fewer than 100 shortcut rows mount at either end, and the last shortcut remains reachable.

Recent resolves at most five pages for an update and does not reload the custom-section cache on a visit. Preferences are written synchronously. This path has no measured open-time performance result. Official Notion documentation supports section reorder and collapse, but the Sky native pipe failed and both video fetches were throttled. No video frames or exact reference dimensions were observed. See [Sidebar sections reference](SIDEBAR-SECTIONS.md).

## Database interaction video study — 2026-11-08

Eleven public walkthroughs were reviewed with scene-by-scene visual and audio analysis. The study covers the ten retained database layouts and shared setup. A parallel source audit compared actual handlers, saved settings, queries and tests with the reported workflows. No app code changed in this research pass.

The [interaction brief](DATABASE-INTERACTIONS.md) sets the implementation order and acceptance paths. The [video notes](../research/notion/DATABASE-VIDEO-REVIEW-2026-11-08.md) retain sources, scene ranges and evidence limits; the adjacent JSON manifest retains analysis IDs. Board drag, explicit date-source setup, full visible-date coverage and direct date manipulation are major gaps. List/Gallery page opening is expected behavior. Chart, Feed, Map and Dashboard still need complete feature paths.

The Dashboard analysis timeline conflicts with indexed chapter metadata, so its exact timing and complete coverage remain unverified. Other video ranges are approximate scene references. Native pointer/keyboard behavior, Undo and performance need separate checks. Tests were not rerun for this documentation-only pass.

## Priority views and page tools — 2026-11-08

| Before | After | Why |
| --- | --- | --- |
| Chart, Map and Dashboard opened unavailable surfaces | Chart and Map query the full filtered source; Dashboard loads independent scoped widgets with saved controls and background work | Build the three user priorities without blocking local editing |
| `/page` had no result | Page creates a child and a positional link in one transaction | Keep child creation available from the block editor and prevent orphan pages |
| Record opening depended on the active database | A shared helper opens a stable database/record ID and keeps the source schema in a peek | Let Map, Chart and Dashboard open their own records safely |
| Simple tables and covers were missing | Covers support local image upload and repositioning; `/table` supports editable cells and Markdown copy/paste | Add the requested page tools without treating a plain table as a database |

The Page block core test passes for atomic creation, stale parent rejection and archive reference remapping. Its UI test creates a child from an uppercase Page search, changes the title, returns to the parent and checks one rendered link. The native Notion host still fails with `Sky Computer Use native pipe startup failed`. This pass therefore uses the prior video analysis plus Notion's official [subpage](https://www.notion.com/en-gb/help/create-a-subpage), [simple table](https://www.notion.com/help/guides/simple-tables-vs-databases), and [cover](https://www.notion.com/help/customize-and-style-your-content) documentation. Exact native appearance and motion are not verified.


The integrated suite passes **373 tests** (278 app, 82 core, 13 engine), with two performance probes ignored. The debug app build, formatting and diff checks pass. The new-page chooser now includes Dashboard. The view chooser keeps Create view outside its scrolling choices so it stays reachable at 850 × 600.

Integrated renderer checks found and fixed a mixed-property Map query failure and a Dashboard Map viewport that ignored the saved source center. Map now clusters overlapping pins and gives each record a stable selection action. Light/1× and dark/2× synthetic Map and editor renders were inspected. Dashboard and Chart light renders were inspected. Automated Map checks keep network fetches disabled; cached tile, corrupt tile, source-center, cluster-selection, stale-response, and cross-source page-open paths have focused regressions.

See [View and page tools](PRIORITY-VIEWS-AND-PAGE-TOOLS.md) for use, saved behavior and bounds. Dashboard record previews remain limited to 100 matching rows per widget. Standalone Chart and Map use full filtered projections, with explicit group and marker limits. Live provider behavior, native Notion appearance, and native frame-rate targets are not verified by this pass.

Dashboard Map cards now keep their content in a bounded scroll viewport. A geometry regression covers 120 pt and 300 pt cards in Edit and View modes and confirms a marker remains reachable. Normal wheel scrolling passes through the standalone Map to the page; Command-scroll zooms. The Map UI regression verifies both paths as separate gestures.

## Map cache, controls and menu repair — 2026-11-08

The Map repair retains decoded tile identities, projects cached imagery with the current camera and requests missing visible tiles. Search/data updates no longer clear the map. Cached parent images cover detail loading during zoom. Wheel and pinch input accumulate and stay anchored at the pointer; saves wait until movement settles. Dashboard maps now move during a drag, defer data queries until release and restore on cancel.

Map fills the database content width. Zoom in, zoom out and Reset are over its top-right corner. Reset streams the full filtered pin extent in the background, with a default world view for an empty source. Search is above the map. The record/location form is in a progressive popup.

Database container headers have no permanent Add cover/Add icon row; their page menu retains customization. The Appearance flyout closes after pointer exit, with a short travel grace and separate keyboard behavior. See [Map interaction repair](MAP-INTERACTIONS.md) for the Before/After review, reference evidence, tests and limits.

The combined suite passes 385 tests (290 app, 82 core, 13 engine), with two performance probes ignored. This does not establish native frame-rate or live-provider performance.

## Timeline fixed column and Map trackpad follow-up — 2026-11-08

Timeline now keeps Name cells at the left edge while dates and bars scroll behind them. The date ruler stays at the top. A shared scroll viewport keeps row names and bars aligned, and the opaque fixed cells block input into the hidden grid. Period controls and New page keep their horizontal position. See [Timeline scroll](TIMELINE-SCROLL.md) and the linked review of three public videos for evidence and limits.

The Map follow-up makes precise two-axis trackpad input pan the standalone map. Pinch and Command-scroll use fractional zoom anchored at the pointer. The tile provider still uses integer tile levels. JSON float decoding now preserves exact stored coordinate values, which prevents a second save from reporting a false concurrent change. Real concurrent changes still retain the unsaved local camera and report an error. See [Map interactions](MAP-INTERACTIONS.md).

Runtime input tests also found a one-event tile lag: tiles used the camera from the previous poll, while pins used the current camera. The common canvas now projects both from the current camera. Tests cover dateline crossing and repeated world copies. The workspace suite passes 388 tests (293 app, 82 core, 13 engine), with two performance probes ignored. Physical trackpad and native frame-rate checks remain blocked.

## Board interactions and map theme — 2026-11-08

Board now supports card drag between groups and subgroups, saved manual order, group controls, card actions, editable card properties, and saved card layouts. A background reader counts the full matching source and supplies bounded per-column windows. Visible-card and column windows limit mounted UI content under one scroll area. Creation supplies guaranteed filter and column values and rejects conflicts before writing. See [Board interactions](BOARD-INTERACTIONS.md) for reference evidence, use and remaining limits.

The default map now uses OpenFreeMap vector features with colors from the requested Snazzy Maps Apple Map Style. The existing tile worker produces bounded SVG tiles; Rune decodes them off the UI thread. City and town labels remain, while road and POI labels are omitted. Navigation controls and pin interactions remain available. This is an interpretation of the reference, with different geometry and label placement. See [Map theme](MAP-THEME.md) for sources, cache bounds and provider terms.

The full workspace suite passes 427 tests: 314 app, 100 core and 13 engine. Three probes are ignored in that suite. The public map fixture probe also passed in a separate run. Light and dark Board renders, card menus and previews, cached map renders, and a cold-cache live map render were inspected. The live check fetched and rendered nine public Perth tiles. Build, formatting and diff checks pass. Native physical input and frame-rate targets remain unverified.

The later Board repair fixes stretched status badges and drag failures. Property rows now support drag while retaining click-to-edit. Drop targets use current drawn geometry and include each column's footer and blank space. View switches retain valid viewport geometry. Card wrapper keys now use record IDs, which prevents a background loaded-record reorder from losing pointer release or cancel. Tests no longer force extra redraws to prepare Board targets. The workspace passes 432 tests, with three probes ignored. See [Board interactions](BOARD-INTERACTIONS.md) for the failure evidence and remaining native-input limit.

## Moss name and app icon — 2026-11-08

| Before | After | Why |
| --- | --- | --- |
| The app, window, settings and bundle used the previous name | They use Moss, including a `Moss.app` bundle and a `.moss` export name | Apply the user's chosen name |
| The app had no custom bundle icon | The bundle and running Dock icon use the supplied block reference with a Sentient-style M | Give the app its requested identity |
| Data and archives use app identifiers | They use Moss identifiers | Apply the chosen name throughout |

Data uses `~/Library/Application Support/Moss`. An explicit `MOSS_DATA_DIR` takes precedence. Archives use the `moss-workspace` format.

The earlier user-facing rename passed 435 tests, with three probes ignored. The ad hoc `dist/Moss.app` bundle passed signature and plist checks. A native test app showed Moss in its window, app menu and settings. The subsequent complete rename updates the Rust package and crate names, source directories, project references, archive format, Markdown table metadata and calculation error markers.

The complete rename passed 436 workspace tests, with three probes ignored. After removing the old environment-variable aliases, all 322 app tests passed again. The project and linked worktree folders were renamed without discarding their changes. The existing data folder was moved to `~/Library/Application Support/Moss`, and its database bytes were verified. A private scratch copy of the existing workspace opened successfully with matching record counts; the original database stayed unchanged during that check. The bundle was rebuilt and its signature verified, and the development app was restarted from the renamed project. Historical reference captures remain untouched.

The final rename removes the compatibility paths at the user's direction. Workspace titles and their search entries use Moss; a private database backup was made before that update. The workspace suite passed 435 tests, with three probes ignored. The app bundle was rebuilt and its signature verified.
