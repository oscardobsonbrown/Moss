# Paper desktop screens

[Open the Desktop screens page](https://app.paper.design/file/01M6TJR715EZ6FZ0K7K6ZRAFNC/p-4-0).

This page contains 22 editable Paper artboards. Their text, shapes, lines and SVG icons are design nodes. They are static visual states, not working controls. No private screenshots or workspace names were uploaded; all names and records are synthetic. The screenshots in `research/notion` remain local.

## Context and scale

The latest live capture context is Notion 7.42.3 on macOS 27.2. The built-in display reports 1800 × 1169 logical points at 2×. The 3600 × 2260 capture is consistent with an 1800 × 1130 logical window, but that window size was not independently measured. The artboards use 1800 × 1130 as a provisional working size. Earlier pass logs have their own version and scale limits. Geist is the requested Moss font; the reference font is not asserted to be Geist.

The Notion examples include inline databases on a document. The artboards preserve that structure: page title, sample heading and body, database title, view toolbar and view content. They omit an observed update warning because it is an installation state, not a product control. Some personal or account features visible in Notion are not part of the agreed local product. The desktop sidebar says Search, removing the source’s AI-oriented “or ask” text under the agreed product exclusion. Source-only component sheets can retain excluded labels as reference.

## Artboards and source states

| Artboard | Local reference | Scope and review |
|---|---|---|
| 01 Light page | `captures/11-empty-page.png`, `captures/pass03-06-populated-table.png` | Light shell, sidebar, tabs and document content. Reviewed at 1×. |
| 02 Table — rest | `captures/pass03-06-populated-table.png`, deep captures 008–010 | Three-column table and status pills. The shared single-line row height is approximately 38 logical px in the current source. Reviewed at 1×. |
| 03 Table — inline name | `captures/pass03-02-name-cell-editor.png`, `CREATE-BEHAVIOR.md` | New row with focused name, blue border and a stable grid. Uses the same provisional 38-unit row height as rest. Reviewed at 1×. |
| 04 Board | `captures/pass03-12-board-populated.png`, `deep-captures/001-shell-board.png` | Status groups with tinted containers and task cards. Reviewed at 1×. |
| 05 Gallery | `captures/pass03-18-gallery-populated.png`, b-075–076 | Medium cards in two columns, blank Page content previews and separate title strips. A New page card completes the second row. Reviewed at 1×. Column count applies to the captured window width. |
| 06 Calendar | `captures/pass03-15-calendar-populated.png` | Month grid with dated sample items and all November days. Reviewed at 1×; final row fits the artboard. |
| 07 Timeline | `captures/pass03-24-timeline-table.png`, b-074 | Wide canvas beyond the page text column, daily ticks, weekend bands, today line, name table, disclosure controls and horizontal scroll indicator. Bars share the date axis. Reviewed at 1×. Exact geometry and drag states remain open. |
| 08 Side peek | `captures/create-07-timeline-range-peek.png` | Right panel with record title, properties and comment area; underlying page reflows into the left area. Reviewed at 1×. |
| 09 Centre peek | `captures/create-04-gallery-centre-peek.png` | Centre panel with a separate editable dimming layer, title, properties and comment area. Reviewed at 1×. |
| 10 Search | `captures/04-search-recent.png` | Rebuilt search field, filters, results, selected row, page preview and shortcuts, with a separate editable dimming layer. Synthetic preview text replaces private source content. Reviewed at 1×. |
| 11 Dark page | `captures/pass05-03-dark-page.png`, `deep-captures/001-shell-board.png` | Dark shell and document. Reviewed at 1×. |
| 12 List | `captures/pass03-20-list-populated.png` | Sample rows and New page. Reviewed at 1×. |
| 13 Chart | `captures/pass03-27-chart-populated.png` | Status bar chart sample. Reviewed at 1×; axes and chart controls are simplified. |
| 14 Feed | `captures/pass03-29-feed-populated.png` | Two sample entries and empty comment fields. Reviewed at 1×; no comment was submitted. |
| 15 Map | `captures/pass04-02-map-empty.png`, `captures/pass04-05-map-pin.png` | No-place control and blank surface. Reviewed at 1×. The source blank map cause is unknown; this is not an offline-state claim. |
| 16 Dashboard access gate | `captures/pass04-22-dashboard-plan-gate.png` | Reference-only depiction of the observed plan gate. Reviewed at 1×. It is not a proposed Moss paywall or a design for dashboard editing. |
| 17 Dark Table — rest | deep captures 008–010 | Dark counterpart of the table with current status pill colours and shared row geometry. Reviewed at 1×. |
| 18 Dark Gallery — medium | b-075–076 | Two-column Medium cards, one synthetic body preview, separate title strips and New page card. Reviewed at 1×. |
| 19 Dark Timeline — month | b-074, b-079 | Dark counterpart of the wide daily timeline. The dependency segment is on Components; the main screen has independent synthetic items. Reviewed at 1×. |
| 20 Library — recents | b-094 | Five-column Recents list with synthetic names, row hover controls, Library tabs and New page. Reviewed at 1×. Other Library views and interactions remain separate evidence. |
| 21 Dark side peek | `captures/create-07-timeline-range-peek.png` | Dark counterpart with separate collapse/expand and page-action vectors. Synthetic property set. Reviewed at 1×; exact pane geometry and reflow remain open. |
| 22 Dark centre peek | `captures/create-04-gallery-centre-peek.png` | Dark counterpart with editable dimming, property rows and page-action vectors. Reviewed at 1×; exact source dimensions and focus states remain open. |

## Review and remaining work

The screenshot reviews checked fit, text hierarchy, contrast and repeated row alignment. The Table rest and edit artboards have matching row heights and column widths; the edit outline is contained inside its cell. The shell now has a 40 px tab bar, a 44 px page toolbar, a sidebar search field, Home control, Customise sidebar row and bottom workspace switcher. Joined New buttons and status pills use shared component geometry. Page actions, view controls, record icons and comment avatars are separate SVG nodes. These changes establish internal consistency and use the captured structure; they do not establish exact pixel equivalence.

The calibration review found timeline overflow and an incorrect one-row Gallery arrangement. Both are corrected. The timeline now has one shared date coordinate system. The Gallery uses two card lanes. Each view has a distinct toolbar icon. The repeated List and Feed page icons, peek comment markers and Map menu caret were also replaced with editable vectors. Table headers include separate Add property and More slots. The inline-edit sample retains the blank Date cell and default Not started status, with one caret directly after the entered title. See [calibration review](PAPER-CALIBRATION.md) for the dated findings and correction notes.

These are working reconstructions. Compare each artboard with a same-theme, same-window capture before treating it as a pixel reference. Panel dimensions, spacing, icon paths, typography and colours still need matched measurement. JPEG source captures prevent exact colour claims. The 38 px table height applies to the measured single-line state; wrapped rows can grow. The Components page contains the separate menu, property, focus, disabled and error samples listed in [its inventory](PAPER-COMPONENTS.md). It does not establish every state. Drag previews, motion and failure recovery still need native study. The centre peek has a sparse synthetic property set. Chart, Map, Dashboard, Feed and List need further source comparison. The Dashboard board is a simplified reference-only interpretation; it changes the source gate copy and omits its illustrations and upgrade control. It is not a faithful reproduction of that source state.

The native 7.42.3 deep pass remains active. Capture failures and stale window observations interrupt it. The user confirmed that only one Notion window is open. These interruptions do not count toward the required continuous hour of active exploration without a new discovery. See [reference coverage](REFERENCE-COVERAGE.md) for the current limits.
