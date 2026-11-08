# Desktop screens: calibration review

Review date: 7 November 2026. Scope: Paper file `Moss — Research and Design`, page `Desktop screens` (`p-4-0`), 17 artboards. This is a read-only review. The Paper file can change after this review.

The references are local Notion captures. The earlier pass 03 and 04 captures use Notion 7.34.0 and an update banner. Deep pass B uses Notion 7.42.3, macOS 27.2, and a 2× display. The screenshot API produced profiled, lossy JPEG data for deep pass B; do not use it to claim exact source colours. Paper uses the user's requested Geist font, so its typeface is an intentional difference. The 1800 × 1130 Paper artboards approximate the study window; this is not a claim of pixel equivalence.

## Findings in priority order

1. **Timeline overflow and structure — `179-0`.** The third event extends past the row and header rules. Paper computed styles show header `1F6-0` contains a 260 px name column plus three 220 px columns, all non-shrinking, inside a 708 px content lane. Event container `1FX-0` has 490 px left padding and its bar `1FY-0` is 160 px wide. Rebuild the timeline as one coordinated, clipped or scrollable viewport with shared axis and row coordinates. The older `captures/pass03-24-timeline-table.png` shows day ticks, weekend bands, a red today line, a separate name-table boundary, and a horizontal scrollbar. Paper currently shows three week labels and omits these controls; exact source dimensions remain unmeasured.
2. **Literal icon glyphs remain.** Replace these with editable SVG icons and separate text lanes: Timeline row names `1FI-0`, `1FP-0`, `1FW-0` and navigation `1F3-0`, `1F4-0`; Gallery names `1BY-0`, `1C6-0`, `1CE-0`; List names `2AE-0`, `2AI-0`, `2AM-0`; Feed titles `2BA-0`, `2BG-0` and comments `2BC-0`, `2BI-0`; Side and Centre peek comments `1Q9-0`, `1QN-0`; Map no-place control `2BM-0`. The repeated footer workspace chevrons are also literal `⌄` text. This list is limited to inspected JSX and is not a complete glyph inventory.
3. **Gallery configuration is not reconciled — `11T-0`.** Paper shows three compact cards on one row with status values. `captures/pass03-18-gallery-populated.png` shows two medium preview cards per row and a large New page tile. A different card size, property visibility, viewport, or version could account for part of this difference. Record the selected configuration before claiming a direct match.
4. **Dashboard is an interpretation — `27N-0`.** The Paper board changes the gate copy and omits the source's three illustrated cards and Upgrade control. `captures/pass04-22-dashboard-plan-gate.png` is the older-version evidence. The simplified board can support local-only planning, but it should not be presented as a faithful capture of that state.
5. **Peek controls need a source-level review — `1GO-0`, `1J0-0`.** Both render very small header controls and use a literal comment marker. The Side peek clips the table behind its pane, which may be appropriate to the viewport but needs comparison at a matching scroll position. `DEEP-PASS-B.md` b-037 records Notion's stated modes; actual centre-opening in this latest pass was not completed. Do not infer exact close-button behavior from these mockups.
6. **Map remains an empty-tile state — `24X-0`.** Paper shows a blank region with a crosshair and No place control. `PASS04.md` notes the native map was blank and the cause was not established. Label this as an observed empty-tile state, not the general Map experience. The editable map control remains text rather than discrete icon and label elements.
7. **Chart, Feed and List represent limited configurations — `1ZH-0`, `227-0`, `1WR-0`.** Screenshots are legible and fit the artboard, but the chart has no visible numeric axis labels, Feed uses glyphs for page/comment marks, and List renders plain status text. Verify each against the corresponding source configuration before extending it to other settings. No claim is made that those choices are always wrong.
8. **Search is a static populated state — `1LC-0`.** The dialog, dimming and preview fit within the artboard. It does not establish keyboard focus, filtering, empty or no-result states. Deep capture b-070 documents New tab search separately; the underlying source includes private recents, which must remain local.

## Checks completed

Screenshots were reviewed for all 17 artboards: light/dark pages, Table rest/edit/dark, Board, Gallery, Calendar, Timeline, Side/Centre peek, Search, List, Chart, Feed, Map, and Dashboard. Table rest and inline editing visually keep the same row height and column boundaries. The Board cards and repeated rows fit in the shown viewport. No artboard-edge clipping was found in the remaining screenshots; the Timeline event overflow above is inside its content lane. This review does not prove all labels, fonts, interaction states, accessibility, responsiveness, or source geometry are correct.

The study has not met its one-hour no-new-findings stopping condition. Do not treat this audit as complete native coverage.

## Corrections after the review

The findings above describe the reviewed state before these corrections. The Desktop screens page now has 22 artboards.

- Timeline now extends beyond the page text column. It has a shared daily date axis, weekend bands, a today line, table boundary, disclosure controls, New row and horizontal scroll indicator. The three synthetic bars fit their rows. A dark counterpart was added. New capture b-074 confirms this wide structure in Notion 7.42.3. Exact spacing, table collapse and drag outcomes remain open.
- Gallery now has two Medium card lanes, separate preview and title regions, and a New page card. A dark counterpart includes one synthetic body preview. b-075–076 confirms the configuration and two-column result at the current window width. b-078 establishes that Small uses three columns at that width; the main screens remain Medium.
- The listed List and Feed page glyphs, comment markers, Timeline name and navigation glyphs, and Map menu caret were replaced with separate editable SVG elements and text. Some other glyphs may remain. Their paths are hand-drawn approximations.
- The shared shell, New split button and status pills were aligned across screens. Search was rebuilt from the component panel, with synthetic page text in place of the source's private preview. These changes were reviewed with Paper screenshots.

Dashboard interpretation, incomplete Chart axes, Map empty tiles, matched peek geometry and the remaining interaction states are still open. Component states have their own [inventory](PAPER-COMPONENTS.md).

Native capture recovered long enough to save b-074–079, then returned unstable window targets and a ScreenCaptureKit stream error. The root agent also received `noWindowsAvailable` on input after a successful observation. Native input is paused until a clear window target is available. Capture outages do not count as discovery-free exploration.

Library Recents was added from b-094 after this review. Its five column lanes, synthetic rows, hover controls and tabs were reviewed at 1×. The Components page has a separate Library row sheet. Source geometry and untested interactions remain open.

Side and Centre peek headers now use separate editable collapse/expand, lock, link, favourite and More vectors in place of combined text glyphs. The source states are create-04 and create-07. The light reconstructions fit and align at 1×; their exact geometry and light/dark comparison remain open. Share is a source reference, not a decision to add network sharing.

Dark Side peek and Centre peek counterparts were added and reviewed. They use shared dark colours and status pills. The review corrected a light tab fill and a low-contrast database heading. They remain synthetic, approximate reconstructions, not proof of exact source geometry.
