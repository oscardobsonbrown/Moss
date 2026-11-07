# Rotion product requirements

Status: implementation baseline. The user has instructed us to start building from the agreed requirements. The reference study is incomplete, and release choices are deferred. Track those limits while implementing the full agreed scope.

Date: 2026-11-07.

## Problem statement

The user wants the power of Notion without network delays or product promotions. Content must open at once. The interface must keep the precise control design and editing behavior that the user values in Notion. Motion must be rare.

The app must support a full personal workspace. A documents-only first release does not meet the request.

## Solution

Build Rotion as a macOS desktop app with AnomalyCo's Rune UI framework. Keep the workspace in an app-managed local database with a folder for attachments. V1 needs no account or sync. Background network fetches are allowed, but they must never block local work.

Include pages and the full database engine: retained database views, properties, formulas, relations, rollups, linked databases, filters, sorting, and templates. The user later excluded Form views from v1. Match exact Notion pixels and behavior for retained features. Record product exclusions, the motion policy, and the Geist font system as deliberate differences. Geist Sans is the UI font. Code uses Geist Mono. Page appearance offers Geist Sans, Geist Serif, or Geist Mono, with Small text and Full width stored per page. Fonts must work without a network connection.

## Audience and distribution

Rotion is a personal workspace app. Publish downloads on the user's website and GitHub so that other people can install and use it. GitHub will contain the full public source code under the MIT license and versioned downloadable app releases. Keep private Notion reference material local and outside the public source repository. Public installation and release readiness are part of the product requirements. Supported Mac hardware and OS versions, update behavior, and release packaging still need decisions.

This records the intended release channels. It does not request publication of the current project or private reference material.

## Confirmed user stories

1. As the workspace owner, I want to start Rotion without an account, so that I can use my data locally.
2. As the workspace owner, I want to open saved content without a connection, so that local work does not wait for a server.
3. As a reader, I want the real first viewport ready for input in less than 8.8 ms, including disk reads, so that opening content does not cause a perceptible wait.
4. As a workspace owner, I want performance tested with 100,000 pages and one database containing 1 million rows, with the agreed higher time limit for that large-database case, so that growth is part of the acceptance tests.
5. As a writer, I want pages and a block editor, so that I can write and arrange rich documents.
6. As a database user, I want table views, so that I can edit records and their properties.
7. As a database user, I want board views, so that I can arrange records in groups.
8. As a database user, I want calendar views inside Rotion, so that I can work with dated records.
9. As a planner, I want timeline and Gantt behavior, so that I can plan work across dates.
10. As a database user, I want all retained Notion database views, so that the app does not force my work into one layout.
11. As a database user, I want typed properties, so that records retain their meaning across views.
12. As a database user, I want formulas, so that values can be calculated from my data.
13. As a database user, I want relations and rollups, so that records can refer to and summarize other records.
14. As a database user, I want linked databases, so that I can show the same source data in several places.
15. As a database user, I want filters and sorting, so that I can find and order relevant records.
16. As a workspace owner, I want templates, so that I can repeat useful structures.
17. As a user familiar with Notion, I want retained controls to match its pixels and behavior, so that the interface remains familiar.
18. As a frequent keyboard user, I want almost no animation, so that the app does not add motion delays to repeated work.
19. As a workspace owner, I want import and export, so that the data remains portable.
20. As a new Rotion user, I want to start with an empty workspace, so that migration of my current Notion workspace is not a release requirement.
21. As a user of external content, I want background fetches to leave local work responsive, so that the network does not control page-open speed.
22. As a user of remote embeds, I accept that they are exempt from the 8.8 ms rule, while the page's local work remains independent of their load.
23. As a user, I want light and dark themes with the system setting as the default, so that Rotion fits my display setting.
24. As a workspace owner, I want local buttons and database automations, so that repeat actions can create pages and change properties without an external service.
25. As a workspace owner, I want recurring templates, so that scheduled work can use a repeat structure. Skip missed occurrences after sleep, shutdown, or explicit Quit.
26. As a user, I want Rotion to keep running after I close its last window, so that normal window opens can use the ready workspace and scheduled local work continues.
27. As a user, I want explicit Quit to stop Rotion and its scheduled work, so that I can stop the resident process when needed.
28. As a user, I want missed schedules skipped, so that resuming Rotion does not run overdue actions.
29. As a workspace owner, I want automatic saving and manual export, so that I can keep my work and make a copy when needed. Page history and scheduled backups are not required for v1.
30. As a user, I want Notion-style trash and restore for deleted pages and records, so that deletion can be reversed without page history or automatic backups.
31. As a workspace owner, I want a lossless Rotion archive, so that I can restore the full workspace, including formulas, relations, view settings, and attachments.
32. As a workspace owner, I want Markdown/CSV, HTML, and PDF exports with a loss report, so that I can use my content in other tools and understand what those formats cannot preserve.
33. As a workspace owner, I want to import Rotion archives, Markdown, CSV, and HTML, so that I can restore Rotion data and add content from other tools.
34. As a user, I want to add images, PDFs, audio, and video as attachments, so that supporting files stay with my content. PDF import does not convert pages into editable blocks.
35. As a workspace owner, I want a Rotion archive to open as a separate workspace, so that restoration does not replace or merge into my current workspace.
36. As a user, I want multiple workspaces open at the same time in separate windows, so that I can compare and use them without closing another workspace.
37. As a user, I want the new Notion desktop tab model, so that each window holds one workspace with a stable sidebar and tabs that preserve scroll and selection.
38. As a user, I want selecting an already open workspace to focus its most recently used window, so that workspace selection does not create another window.
39. As a user, I want Rotion to restore only the last active workspace window and its tabs after Quit, so that other workspace windows stay closed at startup.
40. As a workspace owner, I want scheduled automations to run in every local workspace while Rotion's service is running, so that unopened or closed windows do not stop scheduled work.
41. As a user, I want local images, PDFs, and videos to show saved previews of real file content within 8.8 ms, with controls ready for input, while full-resolution content loads in the background.
42. As a database user, I want a local list of people for assigning responsibility and roles, so that person assignments work without accounts or invitations.
43. As a database user, I want roles to be regular properties beside People, so that a person can have different roles on different records without a separate assignment system.
44. As a map user, I want saved locations, markers, and controls ready within 8.8 ms while map tiles load in the background, with cached tiles available offline.
45. As a user, I want local comments on pages and blocks, with threaded replies and Resolve/Reopen actions, so that I can keep annotations with the relevant content.
46. As a user, I want menus, dialogs, and tabs to animate while database views switch instantly, so that motion follows the specific interaction rules I selected.
47. As a keyboard user, I want menus, dialogs, and tab switches to stay instant when triggered by keyboard, so that frequent commands have no animation delay. Pointer-triggered actions can animate.
48. As a user clicking a tab, I want its content to change immediately while only the active-tab indicator animates, so that tab motion does not delay content readiness.
49. As a user with a database containing 1 million rows, I want its real first viewport ready for input within 20 ms, including disk reads, so that the benchmark covers a single large database.
50. As a user, I want all five selected workflow scenarios tested as separate workspaces, so that readiness covers distinct uses of Rotion.
51. As a user, I want search to use the current workspace by default and offer an explicit All workspaces option, so that I can choose the search scope.
52. As a user, I want search to find page titles, block text, database property values, and attachment filenames, so that I can locate my stored content. PDF contents and text recognition in images are outside v1.
53. As a user, I want local macOS notifications for date reminders while Rotion is running, even with all windows closed, so that deadlines and follow-ups can reach me without an open window.
54. As a user, I want a dialog offering optional Start at login, so that I can enable the background service at macOS login without opening workspace windows.
55. As a user, I want one summary of overdue reminders when Rotion resumes, while missed automations remain skipped, so that overdue work is visible without a notification burst.
56. As a user, I want deleted pages and database records held in Trash for 30 days before permanent removal, so that I have a defined restore period.
57. As a user, I want search to remain inside Rotion rather than use a global keyboard shortcut outside the app.
58. As a user downloading Rotion from the website or GitHub, I want to install and use it as a personal workspace app without an account or sync service.
59. As a developer, I want public source code and versioned app releases on GitHub, so that I can inspect, build, and contribute to Rotion.

These stories record confirmed scope. Each needs detailed behavior and acceptance cases before the PRD is final.

## View inventory

The observed Notion 7.34.0 chooser lists these views:

| View | Rotion scope | Reference state study |
| --- | --- | --- |
| Table | Included | In progress |
| Board | Included | In progress |
| Gallery | Included | Pending detailed states |
| List | Included | Pending detailed states |
| Chart | Included | Pending detailed states |
| Dashboard | Included | Creation reaches a Business plan gate; editing remains blocked |
| Timeline | Included; user also requires Gantt | Dependency and sub-item settings, one forward date shift, and a connector observed. Other shift cases and drag remain untested. |
| Feed | Included | Pending detailed states |
| Map | Included | Setup and generic marker observed; blank map surface cause unknown. Tiles are exempt from the timing target and cached tiles are available offline. |
| Calendar | Included | In progress |
| Form | Excluded from v1 by a later user decision | Builder captured as reference only |

A view label is not a complete feature specification. The state library must cover controls, setup, empty content, populated content, edits, keyboard input, dismissal, errors, and failures where applicable.

## Design and motion requirements

- Use exact reference pixels and behavior for retained features.
- Record reference version, theme, viewport, backing scale, and state for each comparison.
- Include light and dark themes. Follow the system theme by default. Both themes need reference-state coverage.
- Keep a list of deliberate differences. The confirmed differences include removed product surfaces, local storage behavior, no sync or account, and almost no motion.
- Use the Notion study to define control geometry and states. Do not treat the Paper research-board style as the app's final design system.
- Apply Emil Kowalski's principles with the user's explicit motion choices: animate menus, dialogs, and tabs; switch database views instantly.
- Keyboard-triggered menus, dialogs, and tab switches stay instant. Animate pointer-triggered actions. Database view switches stay instant for every trigger.
- On a pointer-triggered tab switch, show content immediately and animate only the active-tab indicator. Do not delay content readiness until the indicator finishes. Keyboard-triggered tab switches have no indicator animation.
- Scrolling, dragging, and standard caret feedback remain allowed. Define effects, durations, interruption, and reduced-motion behavior before implementation. Do not add other motion without a clear requirement.
- Accessibility, focus, IME, text selection, clipboard behavior, and keyboard behavior need acceptance cases. Visual matching alone is insufficient.

## Performance requirements

The ordinary content-open target is below 8.8 ms, including disk reads, on the user's Apple M4 Mac with 16 GiB RAM. The real first viewport must be ready for input. Test a workspace with 100,000 pages and a single database containing 1 million rows. For that large-database case, the real first viewport must be ready for input within 20 ms, including disk reads.

The 8.8 ms rule applies while the app is running, including uncached disk reads. Cold start has a separate 300 ms target from launch request to the real first viewport ready for input, at the same workspace size.

Remote embeds are exempt from this timing target. Their loading must not block local work.

Map tiles are also exempt. Saved locations, markers, and controls must be ready within 8.8 ms. Tiles may load in the background, with cached tiles available offline. Tile loading must not block local work.

Local images, PDFs, and videos may show a saved preview within 8.8 ms while full-resolution content loads in the background. The preview must show real file content, and controls must be ready for input.

The full measurement contract is in [PERFORMANCE.md](PERFORMANCE.md). Clock boundaries, data shape, cache conditions, preview generation and missing-preview behavior, and OS interference handling remain open. No benchmark result exists.

## People and responsibility

People properties use a local list of people for assigning responsibility and roles. Entries do not require an account and do not create invitations or access permissions. Roles are regular database properties beside People. A person can have different roles on different records. A separate role assignment for each person on a record is not required.

## Search

Search uses the current workspace by default and provides an explicit All workspaces option. Index page titles, block text, database property values, and attachment filenames. PDF contents and text recognition in images are outside v1. Do not provide a global keyboard shortcut for search outside Rotion. In-app search remains included.

## Reminders

Use local macOS notifications for date reminders while Rotion is running, including when all windows are closed. Show one summary of overdue reminders when Rotion resumes. Missed automation occurrences remain skipped. Notification access, notification actions, and summary details still need definition.

## Local comments

Include local comments on pages and blocks. Support threaded replies and Resolve/Reopen actions. Comments are annotations stored in the local workspace.

## Trash and retention

Keep deleted pages and database records in Trash for 30 days from deletion, then remove them permanently. Restore remains available during that period. Page history and scheduled workspace backups remain outside v1.

## Data portability

Manual export must support a lossless Rotion archive and Markdown/CSV, HTML, and PDF formats. The Rotion archive must restore the full workspace, including formulas, relations, view settings, and attachments. The other formats must report any losses.

Import must support Rotion archives, Markdown, CSV, and HTML. Images, PDFs, audio, and video are attachments. PDF import does not convert pages into editable blocks.

Restore a Rotion archive as a separate workspace. Do not replace or merge into the current workspace. Multiple local workspaces can stay open at the same time in separate windows. Study Notion's switcher and window behavior before defining the detailed interactions. The behavior when Markdown, CSV, or HTML imports meet existing content still needs a decision. Migration of the user's existing Notion workspace is not required for v1.

## Workspaces and windows

Multiple local workspaces can stay open in separate windows. Archive restore opens a separate workspace.

The native Notion 7.34.0 study observed a bottom-sidebar workspace switcher with a visible active checkmark. A new study window switched to a second workspace while the original window kept its workspace and two tabs. The original window remained after the study window closed. This is evidence for that session, not for restart persistence.

Rotion will use Notion's newer desktop tab model: each window holds one workspace, all tabs in the window belong to that workspace, and the sidebar stays stable. Each tab preserves its scroll and selection. Navigation to another workspace uses another window. Selecting an already open workspace focuses its most recently used window.

After Quit, restore only the last active workspace window and its tabs. Keep all other workspace windows closed. The restored window's real first viewport must be ready for input within the confirmed 300 ms cold-start target.

Offer optional Start at login in a setup dialog. Keep it disabled until the user enables it. When enabled, macOS login starts the service without opening workspace windows. Provide the setting in Settings so that it can be changed later. This startup path is distinct from manually opening the app and restoring the last active window.

While Rotion's service is running, scheduled automations run in every local workspace, including workspaces not opened in the current session. A workspace does not need a visible window for its scheduled work. Explicit Quit stops all scheduled work. Skip missed occurrences. Starting this work must not block the restored window or local input.

The installed classic app and the newer desktop beta remain separate evidence baselines. The classic captures do not establish the beta's exact appearance or every interaction.

See the [workspace findings](../research/notion/WORKSPACES.md) and [native pass 06](../research/notion/PASS06.md) for evidence, official sources, proposed behavior, and open states. Account, membership, billing, and login controls remain outside Rotion v1.

## Candidate module boundaries

These are proposals for discussion, not approved implementation decisions.

| Module | Responsibility | Main behavior to verify |
| --- | --- | --- |
| Local workspace store | Pages, blocks, records, properties, transactions, and attachments | Durable saves, atomic changes, recovery, and referential integrity |
| Document engine | Block structure, text edits, selection, clipboard, and undo | Correct edits, IME, rich paste, cross-block operations, and history |
| Database engine | Types, formulas, relations, rollups, queries, and invalidation | Correct values, cycles, filter semantics, sorting, and shared source consistency |
| View system | Each database layout and its interactions | Equivalent data across views, selection, drag, dates, and view settings |
| Rune interface | Shared controls, tokens, navigation, menus, focus, and accessibility | Reference-state matching and native input behavior |
| Search index | Page and record discovery | Correct results and responsive updates at the required scale |
| Local automation engine | Buttons, event rules, recurring templates, and local action execution | Rule results, scheduling, retries, cycles, and atomic changes |
| App lifecycle | Resident operation with no visible window, reopen, and explicit quit | Continued scheduled work, restored workspace, memory use, and clear quit behavior |
| Data transfer | Import, export, and format validation | Round trips, clear loss reports, and safe handling of invalid input |
| External content | Background fetches and local representations | Offline behavior, stale data, failures, and no blocked local work |

## Testing decisions

The user selected all five scenarios in the [video shortlist](../research/notion/WORKFLOW-VIDEOS.md) as separate acceptance workspaces: project and task management, school planning, content production, sales CRM, and meal planning. The [acceptance workflow draft](ACCEPTANCE-WORKFLOWS.md) defines proposed setup steps and checks using sample data.

Proposed test types:

- State comparisons against a pinned Notion reference, with deliberate differences listed.
- Real editing and database behavior tests. Test user-visible results, not internal function structure.
- Storage and recovery tests for app termination, failed writes, invalid input, and interrupted changes.
- Formula and query tests that include relation changes, cycles, dates, and large datasets.
- Instrumented performance tests. Record every target miss and the slowest result.
- Native macOS checks for focus, keyboard input, IME, accessibility, clipboard, and presentation.

The five workflow scenarios are selected. The user has not yet reviewed module boundaries, module-specific test coverage, or the detailed acceptance steps. There is no existing Rotion test suite.

## Out of scope

- User accounts and workspace sync in v1.
- The separate Notion product suite: AI, Mail, the separate Calendar product, Sites, and team administration.
- Migration of the user's current Notion workspace as a v1 requirement.
- Email, webhooks, and automation actions in other services.
- Form views, form builders, and public form responses.
- Page history and scheduled workspace backups. Trash with restore remains included.
- Searching PDF contents and recognizing text in images.
- A global keyboard shortcut for search outside Rotion.

General import and export, calendar database views, and the full personal database engine remain included.

## Research gates before the final PRD

1. Resolve the remaining questions in [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md).
2. Complete or explicitly classify each required state in the [state matrix](../research/notion/STATE-MATRIX.md).
3. Obtain reference recordings or resolve the recorder access limit. Current evidence contains ordered still images only.
4. Review exact feature checklists for blocks, properties, formulas, templates, every view, and Gantt behavior.
5. Review the module and test proposals with the user.
6. Publish a final PRD only when the decisions and remaining evidence limits are explicit.

## Further notes

The Paper project contains the brief and confirmed decisions. Raw Notion captures remain local because some include private workspace content. Two Terra 6.1 agents performed the computer-use study. The current UI research is partial; it does not establish every state or any timing result.
