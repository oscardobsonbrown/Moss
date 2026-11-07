# Confirmed decisions

Date: 2026-11-07. Source: user interview.

| ID | Decision | Status |
| --- | --- | --- |
| D01 | Name the project Rotion. Use `~/Projects/Rotion`. | Confirmed |
| D02 | Use AnomalyCo's Rune UI framework. | Confirmed |
| D03 | Target macOS desktop first. | Confirmed |
| D04 | V1 uses local workspaces with no account or sync. | Confirmed; archive restore opens a separate workspace under D32 |
| D05 | V1 is the full personal workspace engine. It is not a documents-only release. | Confirmed |
| D06 | Include all database views, properties, formulas, relations, rollups, linked databases, filters, sorting, and templates. | Confirmed; Form views later excluded by D27 |
| D07 | Include calendar views within Rotion. Exclude the separate Calendar product, AI, Mail, Sites, and team administration. | Confirmed |
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
| D22 | The 8.8 ms content-open rule applies while Rotion is running, including uncached disk reads. | Confirmed |
| D23 | Cold start must show the real first viewport ready for input within 300 ms, at the agreed workspace size on this Mac. | Confirmed target; not yet measured |
| D24 | Closing the last window keeps Rotion running. It keeps the workspace ready and runs scheduled local work in the background. | Confirmed; explicit Quit is defined in D25 |
| D25 | Explicit Quit stops Rotion and scheduled work. Closing a window keeps them running. | Confirmed |
| D26 | Skip schedule occurrences missed during sleep, shutdown, or explicit Quit. Do not run them on resume. | Confirmed |
| D27 | Exclude Form views from v1. | Confirmed; overrides the earlier all-views scope for Form |
| D28 | Require automatic saving and manual export for recovery. Do not require page history or scheduled backups in v1. | Confirmed; page and record deletion behavior is being resolved separately |
| D29 | Use Notion-style trash with restore for deleted pages and database records. Do not add page history or automatic backups in v1. | Confirmed |
| D30 | Manual export includes a lossless Rotion archive plus Markdown/CSV, HTML, and PDF. The archive preserves the full workspace, including formulas, relations, views, and attachments. Other formats report losses. | Confirmed |
| D31 | Import Rotion archives, Markdown, CSV, and HTML. Add images, PDFs, audio, and video as attachments. PDF import does not convert pages into editable blocks. | Confirmed |
| D32 | Restore a Rotion archive as a separate workspace. Do not replace or merge into the current workspace. | Confirmed |
| D33 | Allow multiple local workspaces to stay open at the same time in separate windows. Study Notion's workspace behavior to define the switcher and window interactions. | Confirmed; reference study in progress |
| D34 | Use the new Notion desktop tab model: one workspace per window, all tabs in that window belong to it, the sidebar stays stable, and each tab preserves scroll and selection. Navigation to another workspace uses another window. | Confirmed; detailed window reuse behavior remains open |
| D35 | Selecting an already open workspace focuses its most recently used window. | Confirmed |
| D36 | After Quit, restore only the last active workspace window and its tabs. Keep other workspace windows closed. | Confirmed after clarification |
| D37 | While Rotion's service is running, run scheduled automations in every local workspace, including workspaces not opened in the current session. Window visibility does not control scheduling. Explicit Quit still stops all work, and missed occurrences remain skipped. | Confirmed |
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
| D50 | Use local macOS notifications for date reminders while Rotion is running, including when all windows are closed. | Confirmed |
| D51 | Offer optional Start at login in a dialog. Keep it disabled until the user enables it. When enabled, start the service in the background without opening windows. | Confirmed |
| D52 | Show one summary of overdue reminders when Rotion resumes. Missed automation occurrences remain skipped. | Confirmed |
| D53 | Keep deleted pages and database records in Trash for 30 days, then remove them permanently. | Confirmed |
| D54 | Do not include a global keyboard shortcut for search outside Rotion. In-app search remains included with its agreed scope options. | Confirmed |
| D55 | Rotion is intended for personal use and will be publicly available to download from the user's website and GitHub. | Confirmed; source-code visibility and release details still need decisions |
| D56 | Publish the full source code on GitHub with versioned downloadable app releases. Keep private Notion reference material local. | Confirmed; license and release details remain open |
| D57 | License Rotion's source code under MIT. | Confirmed |
| D58 | Start implementation now from the agreed requirements. Defer release-detail questions and resolve routine technical choices during the build. | Explicit user direction |

## Performance intent

The ordinary content-open target is below 8.8 ms, including disk reads, for a real first viewport that is ready for input. Test 100,000 pages and one database containing 1 million rows on this Mac. The first-viewport limit for that large-database case is 20 ms, including disk reads. Offscreen content can load as needed. At 120 Hz, a frame lasts 8.33 ms. Clock boundaries, data shape, cache conditions, OS interference policy, and test method still need detail. No performance result or universal guarantee has been established.

## Scope interpretation that still needs detail

The observed Notion view chooser lists Table, Board, Gallery, List, Chart, Dashboard, Timeline, Feed, Map, Calendar, and Form. All remain in scope except Form, which the user later excluded. Each retained view needs a feature and state checklist. The chooser does not establish the required Gantt behavior.

The capture study is a reference inventory. Captured product-suite controls do not become Rotion features by default.
