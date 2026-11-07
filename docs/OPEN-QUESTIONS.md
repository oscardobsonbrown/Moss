# Open product decisions

The interview is paused at the user's request. Build from the agreed requirements. Resolve routine choices from source evidence and tests; ask only when a missing decision blocks work.

## Current branch: performance

- Ordinary content open must take less than 8.8 ms including disk reads. The first real viewport must be ready for input. Test 100,000 pages and one database containing 1 million rows on this Mac. The first-viewport limit for that large-database case is 20 ms, including disk reads.
- Define the clock boundaries, data shape, OS-cache conditions, and visible-result measurement.
- Define typing, search, and query-update limits. Local images, PDFs, and videos can show saved previews of real content within 8.8 ms, with controls ready for input; full-resolution content can load in the background. Define preview generation, missing-preview behavior, and audio readiness.
- Cold start has a confirmed 300 ms target. The 8.8 ms rule applies while running and includes uncached disk reads. Define controlled launch and cache conditions for the tests.
- Define memory use and cache limits. Decide what must stay local and available after restart.

## Next branches

1. Pin the Notion version, theme, display scale, and viewport for exact pixel and behavior matching. Product exclusions and almost no motion are deliberate differences.
2. Define the full block list, nested blocks, columns, linked blocks, mentions, backlinks, and embedded content. Local page and block comments are included, with threaded replies and Resolve/Reopen actions. Define comment behavior when the referenced content changes or is deleted.
3. Define every retained database view. Resolve Feed, Chart, Dashboard, and remaining Map details within a local-only app. Map tiles are exempt from the 8.8 ms target; saved locations, markers, and controls must meet it, and cached tiles are available offline. Form views are excluded from v1.
4. Define Gantt dependencies, date propagation, milestones, grouping, and zoom behavior.
5. Define property types, formula compatibility, relations, rollups, filters, sorting, grouping, and templates. People properties use a local list for responsibility, without accounts or invitations. Roles are regular database properties beside People.
6. Define conflicts for Markdown, CSV, and HTML imports, export selection, and loss reports. Rotion archive restore opens a separate workspace. Confirmed imports: Rotion archives, Markdown, CSV, and HTML, with images, PDFs, audio, and video as attachments. Confirmed exports: lossless Rotion archive, Markdown/CSV, HTML, and PDF. Migration of the user's existing Notion workspace is not required for v1.
12. Define navigation, settings scope, and the enabled state of automations restored from an archive. The user chose the new Notion desktop model: one workspace per window, a stable sidebar, and preserved per-tab scroll and selection. Selecting an already open workspace focuses its most recently used window. Multiple local workspaces can stay open, and archive restore opens a separate workspace. After Quit, restore only the last active workspace window and its tabs. All local workspaces run scheduled automations while the service runs, even if their windows stay closed.
7. Define save, crash recovery, restore, undo, corruption, disk-full behavior, and Trash edge cases. Trash holds deleted pages and records for 30 days before permanent removal. Automatic saving and manual export are included; page history and scheduled backups are excluded.
8. Define search ranking and filters, in-app keyboard shortcuts, tabs, windows, navigation history, and command menus. Search uses the current workspace by default and provides an explicit All workspaces option. Index page titles, block text, database property values, and attachment filenames; PDF contents and text recognition in images are outside v1. Do not add a global search shortcut outside Rotion.
9. Define accessibility, IME input, text size, and motion details. Pointer-triggered menus, dialogs, and tabs animate. Keyboard-triggered actions and all database view switches stay instant. Define effects, durations, interruption, and reduced motion. Light and dark themes follow the system setting by default.
10. Define release gates, delivery order within the full scope, and module test requirements. All five video scenarios are selected as acceptance workspaces; review the detailed steps in ACCEPTANCE-WORKFLOWS.md.
11. Define local automation triggers, supported actions, cycles, and retries. Closing the last window keeps the app and scheduled work running. Explicit Quit stops them. Skip missed schedules. External service actions are excluded from v1.
13. Define reminder notification actions, overdue-summary details, and setup-dialog timing. Date reminders use local macOS notifications while Rotion is running, even with all windows closed. Show one overdue-reminder summary on resume and skip missed automations. Offer optional Start at login in a dialog; it stays disabled until enabled, and login startup runs the service without opening windows.
14. Define public release details. Rotion is for personal use, with MIT-licensed public source code and versioned downloads on GitHub and app downloads on the user's website. Keep private Notion reference material local. Resolve Mac hardware and OS support, update behavior, packaging, and release checks. The user approved MIT. The current copyright notice names Rotion contributors; final public release details remain deferred.

## PRD status

The PRD is the implementation baseline. The user authorized implementation on 2026-11-07. The state study and some detailed behavior remain open. GitHub and website publication are deferred.
