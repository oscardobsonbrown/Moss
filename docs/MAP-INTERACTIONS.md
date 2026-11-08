# Map interaction repair — 2026-11-08

## Cause and change

| Before | After | Why |
| --- | --- | --- |
| Each camera, record or search change cleared the visible tiles | A shared loader retains up to 128 decoded images by provider and tile coordinate | Reuse loaded images and request only missing visible tiles |
| Tiles retained screen positions from an old camera | Geographic tiles are projected from the current camera, with stable keys for each world copy | Keep tiles and pins together during movement |
| Zoom removed the current map while new tiles loaded | Cached parent tiles remain under ready detail tiles | Keep available map content visible |
| A precise wheel tick changed a full zoom level and wrote settings | Pinch and Command-scroll use fractional zoom anchored at the pointer; position saves after 180 ms without change | Avoid sudden zoom steps and a write on every tick |
| Two-finger gestures scrolled the page instead of the map | Precise two-axis scroll input pans the map directly | Keep trackpad movement on its visible target |
| A saved coordinate could report a false conflict on the next move | JSON decoding preserves the exact stored floating-point value | Prevent number conversion from being treated as a change in another window; retain real conflict checks |
| Dashboard maps moved only after pointer release | A transient camera follows the drag and restores on cancel; source queries wait for release | Keep movement direct without restarting all widget queries |
| Custom Search places and Place / Set location rows sat above the map | Both rows are removed; map navigation stays over the map | Follow the user's request to remove the added rows |
| Zoom controls sat outside the map | Zoom out, Zoom in and Reset float inside its top-right corner; Reset uses Phosphor Corners Out | Make their target clear and keep the requested control order |
| Reset could only use the visible marker page | A bounded worker streams the full matching source extent, including records beyond the 500-marker limit | Fit matching pins outside the current viewport; use a world view when none match |
| The map used Table's horizontal scroll wrapper and could shrink | Map uses a vertical scroll viewport and fills the database width | Preserve the normal content width |

A two-finger trackpad gesture over the standalone map pans it on both axes. A notched mouse wheel moves the page. Command-scroll, native pinch and the zoom buttons change map zoom. Drag pans. Fractional zoom scales the map continuously while the provider still uses integer tile levels and the existing bounded image cache. The separate Search places and Place / Set location rows have been removed. Saved locations and the map's navigation controls are retained.

Reset rejects a response after a newer gesture or source/view change. Extent calculation uses bounded memory. Its longitude extent is conservative at the dateline, so Reset can zoom out farther than necessary there. A single pin has a maximum fit zoom of 14. Dashboard's Reset restores its saved source camera.

A failed tile fetch can be retried explicitly without dropping ready images. No offscreen network prefetch was added. The later [Map theme pass](MAP-THEME.md) replaces the default raster provider with themed OpenFreeMap vector features and records the new shared disk-cache bounds. The interaction tests keep network access disabled; the theme pass also has a separate live-provider check.

## Checks and limits

The combined suite passes 388 tests: 293 app, 82 core and 13 engine. Two performance probes remain ignored. The 18 focused Map tests cover cached pan and zoom-back reuse, stable image IDs, parent fallback, stale work, explicit retry, pointer-anchored fractional zoom, diagonal trackpad pan, delayed position saves, and live Dashboard tile/pin movement with drag cancellation. They also cover exact repeated coordinate saves, true conflicts, save/reopen, later gestures after a lost pinch-end event, and same-frame tile/pin movement across repeated world copies and the dateline.

The extent regression covers 526 records with scalar Status/Text siblings. The UI workflow checks full canvas width, removed rows, control order, top-right controls, Reset and menu access at 1× and 2×. The Appearance flyout tests cover both opening sides, diagonal travel, pointer leave and keyboard dismissal.

Local diagnostic renders use generated cached PNG grids under `/tmp/moss-map-repair-review`; these are test patterns, not provider map imagery. Retained images allow Rune to reuse decoded pixels. This does not mean that every GPU upload is avoided: a new mip level or an evicted atlas page can still require one. Native frame-rate and live-provider behavior were not measured in this repair.

The trackpad regressions inject native-shaped events into Rune's runtime. They do not replace a check with a physical trackpad. Native computer use still fails at Sky pipe startup.

## Database header and Appearance follow-up

Database container headers show no persistent Add cover or Add icon row. Page icon and Page cover remain in the page menu; existing covers are retained. Ordinary document and record-page controls remain separate.

Notion's official [page style guide](https://www.notion.com/en-gb/help/customize-and-style-your-content) supports database covers and page icons and describes adding them from hover controls. Moving Moss's database customization actions into its page menu follows the user's request to remove the permanent header controls; it is a deliberate placement choice, not a claim of exact native parity. Native inspection still fails with the Sky computer-use pipe startup error.

Appearance now tracks pointer entry and exit for both its trigger and panel. A 120 ms grace supports movement across the gap or briefly across a sibling row. Leaving both areas closes the flyout and retains the clicked parent menu. Keyboard opening stays active until Left, Escape or an outside action; a second Escape closes the parent.
