# Database views and page tools — 2026-11-08

## Use

- Create a saved Chart, Map or Dashboard from the database view controls. These layouts now open their own working surfaces. Dashboard is also available in the new-page chooser.
- Chart settings select bar, line, donut or number output, category and value properties, aggregation, date buckets and an optional series. Select a point to inspect its records; drilldown reads use bounded pages.
- Map uses a Place property with a name, latitude and longitude. The separate Search places and Place / Set location rows were removed at the user's request. Drag or two-finger trackpad input pans the map. Pinch, the buttons or Command-scroll zoom it. A notched mouse wheel moves the page. A marker opens its record. Overlapping markers form a cluster that lists each record. Saved places work without a network connection. Cached tiles remain available offline.
- Dashboard has Edit and Done states. Add a widget from a saved source view, then move, resize, duplicate or remove it. Each widget uses its own source query and stable record IDs. Source links open the complete source view. Global and widget filters add typed equality rules without changing the saved source.
- Add cover opens an anchored picker. Choose a color or gradient, or upload a local PNG, JPEG or WebP. Image covers support Reposition, Save position and Cancel. Remove cover clears the banner. Failed saves retain the draft.
- `/page` creates a child page and a positional page link in one transaction. A stale parent cannot create an orphan child. Child titles and icons refresh in the parent.
- `/table` creates a simple text grid. Select a cell to edit it; Tab and Shift-Tab move between cells. Row and column controls insert or delete with confirmation. Header row and Header column change the grid styling. Copy Markdown exports the grid. Paste a pipe table into an empty paragraph to create a grid, or into a focused grid cell to replace the grid.

## Bounds

Chart aggregates the complete filtered query on a worker, with at most 128 category/series groups. A larger result gives an error. Its standalone record drilldown pages through the matching category.

Map projects the complete filtered query in its viewport and shows at most 500 markers with a truncation notice. It can scan a large sparse database on a worker; an in-flight standalone Map query is not interrupted. This path has no measured million-row performance result. Tile and explicit place-search requests run in the background. OpenStreetMap is the default tile provider; Photon is the default search provider. `MOSS_MAP_TILE_URL` and `MOSS_PLACE_SEARCH_URL` can change them. `MOSS_MAP_NETWORK=off` disables fetches and keeps cached tiles available. Automated tests do not contact either provider.

Dashboard record widgets preview up to 100 matching records. They render the selected Table, Board, Calendar, Timeline, Gallery or List layout and provide a source link. Chart widgets use the full aggregate projection but offer bounded drilldown samples. Map widgets have separate viewport and tile state, start from the saved source center and zoom, and reset when that source changes. Widget previews do not include every tool from a standalone view. At most 12 widgets are saved; equality is the current exposed filter operator.

Simple tables support 200 rows, 30 columns, 16 KiB per cell and 1 MiB of text per table. They are separate from database tables. Turning a grid into an ordinary block is blocked to preserve its contents. The lossless workspace archive retains grid data, child-page links and cover assets.

Cover uploads are limited to 12 MiB, validated as decoded images, content-hashed and stored in the managed attachment folder. Image loading uses a bounded worker queue and cache. The archive carries deduplicated cover assets and preserves the checksum format of old archives without covers.

## Evidence

The previous scene-by-scene video study is recorded in [Database interactions](DATABASE-INTERACTIONS.md). Official Notion references establish [child page creation](https://www.notion.com/en-gb/help/create-a-subpage), [simple table controls](https://www.notion.com/help/guides/simple-tables-vs-databases), and [cover controls](https://www.notion.com/help/customize-and-style-your-content). Native Notion inspection remains blocked by the Sky computer-use pipe startup failure. Exact native pixels and motion are not verified.

Synthetic review data and renderer captures are local in `/tmp/moss-priority-views-20261108`. No live-provider or native frame-rate result is claimed.
