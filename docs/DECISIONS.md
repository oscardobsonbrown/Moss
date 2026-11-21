# Confirmed decisions

Date: 2026-11-07. Source: user interview.

| ID | Decision | Status |
| --- | --- | --- |
| D01 | Name the project Moss. Use `~/Projects/Moss`. | Confirmed |
| D02 | Use AnomalyCo's Rune UI framework. | Confirmed |
| D03 | Target macOS desktop first. | Confirmed |
| D04 | V1 uses local workspaces with no account or sync. | Confirmed; archive restore opens a separate workspace under D32 |
| D05 | V1 is the full personal workspace engine. It is not a documents-only release. | Confirmed |
| D06 | Include all database views, properties, formulas, relations, rollups, linked databases, filters, sorting, and templates. | Confirmed; Form views later excluded by D27 |
| D07 | Include calendar views within Moss. Exclude the separate Calendar product, AI, Mail, Sites, and team administration. | Confirmed |
| D08 | Store data in an app-managed local database with an attachment folder. Provide import and export. | Confirmed |
| D09 | Make the design system a primary requirement. Remove product promotions and excess interface elements. | Confirmed |
| D10 | Use almost no animation. Apply Emil Kowalski's principles. | Confirmed |
| D11 | Use Terra 6.1 agents with computer use for the Notion state study. | Started: two agents |
| D12 | Create a Paper project for research and design. | Created; link in README |
| D13 | A content open must show the real first viewport, ready for input. Offscreen content can load as needed. A blank shell, spinner, or placeholder fails this requirement. | Confirmed |
| D14 | Ordinary content open must take less than 8.8 ms, including disk reads. Do not replace this with a cache-only target. | Confirmed target; D46 defines the large-database exception; not yet measured |
| D15 | Test the performance target at 100,000 pages and 1 million database rows on this Mac. | Confirmed scale |
| D16 | Match exact Notion pixels and behavior for retained features. | Confirmed; motion and product exclusions still apply |
| D17 | The user can start with a new workspace. Migration of the existing Notion workspace is not required for v1. | Confirmed; general import and export remain included |
| D18 | Background network fetches are allowed. They must never block local work. | Confirmed |
| D19 | Remote embeds are exempt from the 8.8 ms target. | Confirmed; local page work must still not wait on them |
| D20 | V1 includes light and dark themes. Follow the system theme by default. | Confirmed |
| D21 | Include local buttons and automations, including local record changes, page creation, and recurring templates. Exclude email, webhooks, and other service actions from v1. | Confirmed; see D24–D26 for lifecycle and missed schedules |
| D22 | The 8.8 ms content-open rule applies while Moss is running, including uncached disk reads. | Confirmed |
| D23 | Cold start must show the real first viewport ready for input within 300 ms, at the agreed workspace size on this Mac. | Confirmed target; not yet measured |
| D24 | Closing the last window keeps Moss running. It keeps the workspace ready and runs scheduled local work in the background. | Confirmed; explicit Quit is defined in D25 |
| D25 | Explicit Quit stops Moss and scheduled work. Closing a window keeps them running. | Confirmed |
| D26 | Skip schedule occurrences missed during sleep, shutdown, or explicit Quit. Do not run them on resume. | Confirmed |
| D27 | Exclude Form views from v1. | Confirmed; overrides the earlier all-views scope for Form |
| D28 | Require automatic saving and manual export for recovery. Do not require page history or scheduled backups in v1. | Confirmed; page and record deletion behavior is being resolved separately |
| D29 | Use Notion-style trash with restore for deleted pages and database records. Do not add page history or automatic backups in v1. | Confirmed |
| D30 | Manual export includes a lossless Moss archive plus Markdown/CSV, HTML, and PDF. The archive preserves the full workspace, including formulas, relations, views, and attachments. Other formats report losses. | Confirmed |
| D31 | Import Moss archives, Markdown, CSV, and HTML. Add images, PDFs, audio, and video as attachments. PDF import does not convert pages into editable blocks. | Confirmed |
| D32 | Restore a Moss archive as a separate workspace. Do not replace or merge into the current workspace. | Confirmed |
| D33 | Allow multiple local workspaces to stay open at the same time in separate windows. Study Notion's workspace behavior to define the switcher and window interactions. | Confirmed; reference study in progress |
| D34 | Use the new Notion desktop tab model: one workspace per window, all tabs in that window belong to it, the sidebar stays stable, and each tab preserves scroll and selection. Navigation to another workspace uses another window. | Confirmed; detailed window reuse behavior remains open |
| D35 | Selecting an already open workspace focuses its most recently used window. | Confirmed |
| D36 | After Quit, restore only the last active workspace window and its tabs. Keep other workspace windows closed. | Confirmed after clarification |
| D37 | While Moss's service is running, run scheduled automations in every local workspace, including workspaces not opened in the current session. Window visibility does not control scheduling. Explicit Quit still stops all work, and missed occurrences remain skipped. | Confirmed |
| D38 | Local images, PDFs, and videos can show a saved preview within 8.8 ms while full-resolution content loads in the background. The preview must show real file content, and controls must be ready for input. | Confirmed |
| D39 | People properties use a local list of people for assigning responsibility and roles. No accounts, invitations, or access permissions are required. | Confirmed; role representation is defined in D40 |
| D40 | Roles are regular database properties beside People. A person can have different roles on different records. A separate role assignment for each person on a record is not required. | Confirmed |
| D41 | Map tiles are exempt from the 8.8 ms target. Saved locations, markers, and controls must be ready within 8.8 ms. Tiles may load in the background, with cached tiles available offline. | Confirmed |
| D42 | Include local comments on pages and blocks, with threaded replies and Resolve/Reopen actions. Use them as workspace annotations. | Confirmed |
| D43 | Animate menus, dialogs, and tabs. Switch database views instantly. Scrolling, dragging, and standard caret feedback remain allowed. | Confirmed correction; animation triggers and values still need detail |
| D44 | Keyboard-triggered menus, dialogs, and tab switches stay instant. Animate pointer-triggered menus, dialogs, and tabs. Database view switches remain instant for every trigger. | Confirmed |
| D45 | When a tab is clicked, show its content immediately and animate only the active-tab indicator. Keyboard tab switches remain fully instant. | Confirmed |
| D46 | Test a single database with 1 million rows. Its real first viewport must be ready for input within 20 ms, including disk reads. Keep the 8.8 ms target for ordinary content. | Confirmed exception; supersedes the earlier 8.8 ms limit for this large-database case |
| D47 | Use all five video scenarios as acceptance workspaces: projects and tasks, school planning, content production, sales CRM, and meal planning. | Confirmed |
| D48 | Search the current workspace by default. Provide an explicit All workspaces option. | Confirmed |
| D49 | Search page titles, block text, database property values, and attachment filenames. PDF contents and text recognition in images are outside v1. | Confirmed |
| D50 | Use local macOS notifications for date reminders while Moss is running, including when all windows are closed. | Confirmed |
| D51 | Offer optional Start at login in a dialog. Keep it disabled until the user enables it. When enabled, start the service in the background without opening windows. | Confirmed |
| D52 | Show one summary of overdue reminders when Moss resumes. Missed automation occurrences remain skipped. | Confirmed |
| D53 | Keep deleted pages and database records in Trash for 30 days, then remove them permanently. | Confirmed |
| D54 | Do not include a global keyboard shortcut for search outside Moss. In-app search remains included with its agreed scope options. | Confirmed |
| D55 | Moss is intended for personal use and will be publicly available to download from the user's website and GitHub. | Confirmed; source-code visibility and release details still need decisions |
| D56 | Publish the full source code on GitHub with versioned downloadable app releases. Keep private Notion reference material local. | Confirmed; license and release details remain open |
| D57 | License Moss's source code under MIT. | Confirmed |
| D58 | Start implementation now from the agreed requirements. Defer release-detail questions and resolve routine technical choices during the build. | Explicit user direction |
| D59 | Use Geist Sans for the UI and Geist Mono for code. Page appearance offers Default, Geist Serif, and Geist Mono, with Small text and Full width. Save these per page. Follow Notion’s Appearance & layout menu behavior. Bundle fonts for offline use. | Explicit user direction; deliberate font difference from Notion |
| D60 | Database creation follows the retained Notion view. Table and List footer actions create inline titles; Board creates inline cards in the selected group; Gallery and Calendar open record pages; Timeline supports undated inline rows and date ranges from a drag. The toolbar New action opens a record page. Do not use a generic Edit record dialog for these paths. | Explicit user correction; native Notion study on 2026-11-07 |
| D61 | Run implementation agents during the Notion audit. Use the same checkout and build. Agents can message each other and send uncertain Notion behavior to the root agent for reference checks. | Explicit user direction; supersedes the proposed separate worktrees for this work |
| D62 | Round the editor pane's top-left corner. Start the tab strip to the right of the sidebar split. Use thinner tabs with a subtle selected background, based on the supplied Paper app screenshot. | Explicit user direction; deliberate shell design difference from Notion |
| D63 | Allow the sidebar divider to be dragged to change its width. Keep the tab strip aligned with the divider and resize page and database content with the remaining space. | Explicit user direction; save the chosen width per local workspace |
| D64 | New page opens a saved page in a new tab with inline Page and Database choices. Show Dashboard only when dashboarding works. Remove Home, place New page below Search, remove the footer New page and Private plus, and align sidebar icons with Private. | Explicit user direction; replaces the New page dialog and previous sidebar layout |
| D65 | Use a compact table Open control with the tab control's styling, fully rounded database view buttons, vertically centered search text, and a matching New split button. Its dropdown offers Empty item and Side peek / Center peek / Full page for this pass. | Explicit user direction; template support remains in the full v1 scope |
| D66 | Fix text inputs across the app through shared sizing, typography, padding, caret, selection, focus and error styles. | Explicit user direction; keep content fonts and wrapped document editing |
| D67 | Keep New blue while retaining its rounded split-button design and effects. Increase horizontal padding on database view pills. | Explicit user correction; view pills use 14-point side padding |
| D68 | Add property-type icons and a visibly separate Table header row. | Explicit user direction; retain existing data row and column geometry |
| D69 | Make placeholders more subtle, including large title placeholders. | Explicit user direction; use separate title and ordinary input hint colors |
| D70 | Keep block action handles reachable when the pointer leaves the text. Preserve the selection when opening the menu. | Explicit user bug report; shared gutter hover and focused/open-menu visibility |
| D71 | Replace the workspace dialog with a Notion-style popup anchored above the sidebar workspace control. | Explicit user direction; retain local workspace actions and omit account/product surfaces |
| D72 | Use consistent, separate rounded sidebar page highlights with instant hover changes. | Explicit user bug report |
| D73 | Size the sidebar New page button to its icon and text, with balanced side padding. | Explicit user direction |
| D74 | Add database columns from the Table header plus; rename and delete from each header popup. Protect the required Title property and confirm deletion of all column values. | Explicit user request to implement column controls |
| D75 | Align property popup field text, section labels, icon boxes, type names and check marks through shared insets and row metrics. | Explicit user polish request |
| D76 | Use red text for Move to Trash. Add a sidebar page context menu that acts on the clicked page without switching the current page. | Explicit user direction |
| D77 | Blink focused text input carets, including selected text, with normal editing feedback. | Explicit user bug report; pinned Rune draws a steady caret, so use the shared input layer and scheduled wakes |
| D78 | Implement the retained Notion Table header controls and live Formula, Relation and Rollup properties through the shared compact dropdown system. | Explicit implementation request; source-pixel and full formula compatibility remain validation gates |
| D79 | Install Times Newer Roman’s regular, bold, italic and bold italic styles for page Serif mode. | Historical font direction; superseded by D87 |
| D80 | Restore full-width sidebar New page styling. Make only the Table footer New page action fit its icon and text. | Explicit correction of D73: the earlier width request referred to the Table footer |
| D81 | Default database page icons are blank for metadata-only records and have two lines when the body contains content. Keep custom icons. | Explicit user direction; apply across database views |
| D82 | Run million-row UI/frame tests and assess Rune virtualization with a dedicated performance agent. | Explicit user request; report measurements and failed targets without assuming success |
| D83 | Match the supplied Paper light-mode tabs: white active surface, subtle border and shadow, grey inactive tabs, active close visible at rest and inactive close on hover. | Explicit user request; keep dark styling and the existing tab motion rules |
| D84 | The million-row Table test must expose all one million records in one logical scroll range. A 100- or 999-row page in a million-record database is only a paged baseline. | User correction; use bounded storage windows and mounted rows, and report unsupported query/layout cases |
| D85 | Use icons for database views, show a More count only when saved view tabs do not fit, and use a separate two-slider control for view management and creation. | Explicit user request; native inspection currently blocked, supplied screenshots and official documentation are the reference |
| D86 | New Table records join the table directly with no New items heading or section gap. | Explicit correction of the interim virtual-window presentation |
| D87 | Use Sentient by Fontshare for page Serif mode; keep Geist Sans UI and Geist Mono code. | Explicit user request; four faces installed locally, excluded from the public package, with bundled Geist Serif fallback |
| D88 | Use a Notion-style shared sidebar/titlebar control row, Search below it, and working back/forward controls before the existing thin tabs. | Explicit user direction; keep sidebar resizing, page corner and Paper-style tab surfaces |
| D89 | Review the sidebar and implement useful page customization with emoji, saved Favorites, clear row actions and consistent icon/title alignment. | Explicit user direction; native picker and sidebar parity remain unverified while the desktop tool host fails |
| D90 | Use Phosphor Regular as the interface icon set, with shared sizes and alignment. Keep user page emoji separate. | User approved the proposal; bundled SVGs render directly through Rune |
| D91 | Add a thin titlebar divider between the sidebar New page icon and the back/forward controls, based on the supplied Codex reference. | Explicit user request; centered on the sidebar boundary and hidden when the sidebar is collapsed |
| D92 | Align page icon, title and ordinary blocks on one content edge; use the supplied editor references to improve block insertion, command search, keyboard editing and gutter controls. | Explicit user request; lists and callouts retain structural insets, native parity remains unverified |
| D93 | Remove New page below Search and retain the header compose control. Keep a 56 pt collapsed icon rail with loaded page and Favorites icons and 20 × 1 pt group rules. Hover reveals the sidebar over content without reflow; use a 120 ms leave grace, Escape dismissal and keyboard pinning. | Explicit user direction; supersedes the sidebar New page placement in D64 and D80. ChatGPT comparison was blocked; dimensions are not measured reference values. |
| D94 | Show the empty-block “Press / to insert blocks” hint only while the block is hovered or focused. Hide the hint when text is present. | Explicit user direction; retain the empty input's click target and caret. |
| D95 | Add per-workspace custom shortcut sections with create, rename, remove, drag and menu reorder, and collapse. Show four unique actual page opens in Recent, excluding startup restoration; support Favorites pages and subpages. Place trailing page actions with a 3 pt offset. | Explicit user direction; native and video interaction parity remains unverified. |
| D96 | Hide sidebar page and section actions at rest. Reveal them on row hover and strengthen their ink and background on direct hover. Keep them visible during keyboard navigation. Use vertical inward arrows for Collapse section and outward arrows for Expand section. | Explicit user direction; retain action hit targets while their paint is hidden. |

| D97 | Prioritize functional Chart, Dashboard and Map views in parallel. Add page cover banners, a Page slash command that creates a child page, and editable simple table blocks. Keep simple tables separate from database Table views. | Explicit user direction; native reference access is blocked, so use the recorded video study and official documentation with stated gaps. |
| D98 | Keep the Timeline Name column fixed horizontally while dates and bars scroll behind it. Use one vertical scroll range to retain row alignment. | Explicit user direction; three public videos support the separate table region but do not prove exact scroll or sticky-header behavior. |
| D99 | Use precise two-axis trackpad scrolling to pan the standalone map, with smooth pointer-anchored pinch zoom and delayed position saves. | User reported failed Map movement and trackpad faults; notched mouse wheels retain page scrolling unless Command is held. |
| D100 | Add functional Board card movement, subgroups, group controls, card actions and saved layout settings. Use bounded background reads and visible UI windows. | User requested repair of the incomplete Board interactions; remaining limits are recorded in BOARD-INTERACTIONS.md. |
| D101 | Theme the default OSM map with colors from the supplied Snazzy Maps Apple Map Style. Use native rendering of OpenFreeMap vector features and retain the map controls. | User supplied the reference; its Google Maps style array cannot style existing OSM PNG tiles. Geometry and label placement are an explicit difference. |
| D102 | Use a 40 pt top bar and a 44 pt collapsed sidebar. Keep native titlebar setup and sidebar hover bounds aligned with their respective dimensions. | User refined the top and left dimensions separately; supersedes the 56 pt rail width in D93. |
| D103 | Use Moss for the app, project and crate names, data folder, configuration and file formats. Use a black block icon with a Sentient-style M based on the supplied reference. | Explicit user direction. |
| D104 | Add collaboration, including local agent access, presence, comments and page history. Show people as circles and agents as hexagons. Use yellow highlights and underlines for commented text, with markers in the right gutter. Do not add suggestion mode. | Explicit user direction; supersedes the page-history exclusions in D28 and D29. The current implementation and its limits are in IMPLEMENTATION.md. |



## Performance intent

The ordinary content-open target is below 8.8 ms, including disk reads, for a real first viewport that is ready for input. Test 100,000 pages and one database containing 1 million rows on this Mac. The first-viewport limit for that large-database case is 20 ms, including disk reads. Offscreen content can load as needed. At 120 Hz, a frame lasts 8.33 ms. Clock boundaries, data shape, cache conditions, OS interference policy, and test method still need detail. No performance result or universal guarantee has been established.

## Scope interpretation that still needs detail

The observed Notion view chooser lists Table, Board, Gallery, List, Chart, Dashboard, Timeline, Feed, Map, Calendar, and Form. All remain in scope except Form, which the user later excluded. Each retained view needs a feature and state checklist. The chooser does not establish the required Gantt behavior.

The capture study is a reference inventory. Captured product-suite controls do not become Moss features by default.

Reference note for D59: Notion 7.42.3 captures b-062–b-064 show page fonts in inline database content as well as page content. Small text appears to reduce page text while table text appears similar; this is provisional and unmeasured. See [Appearance](APPEARANCE.md) for the remaining comparison. This evidence does not change the selected Geist fonts.
