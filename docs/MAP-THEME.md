# Native map theme

Moss uses an OSM-based interpretation of the requested [Apple Map Style](https://snazzymaps.com/style/241528/apple-map-style). That reference is a Google Maps feature-style array. It cannot be applied to the old pre-rendered OSM PNG tiles.

The default source is now [OpenFreeMap](https://openfreemap.org/). Its OpenMapTiles vector features are decoded on the existing map tile worker. Moss colors the features and writes a small SVG tile. Rune rasterizes that SVG on its image worker. Camera movement, pin placement and the retained image cache are unchanged.

| Before | After | Why |
| --- | --- | --- |
| Fixed OSM raster cartography | Feature colors based on the requested reference | Style land, water and roads separately |
| Dense road and POI labels | City and town labels remain; road and POI labels are omitted | Keep the map clear around database pins |
| External map-style APIs would need a key | OpenFreeMap vector tiles and native rendering | Keep the map usable without an account, paid service or WebView |

The main colors are warm land `#f7f1df`, natural areas `#d0e3b4`, parks `#bde6ab`, water `#a2daf2`, medical areas `#fbd3da`, airports `#cfb2db`, yellow highways `#ffe15f` with `#efd151` edges, and white arterial roads. Local roads use a softer dark neutral. Building shapes, label placement and geometry follow OpenMapTiles rather than Google. This is not an exact Apple or Google map reproduction.

City/town names use the bundled Geist font, converted to glyph paths on the worker. Names prefer English, then the Latin-name field, then the source name. Unsupported glyphs cause that label to be omitted. Road and label widths use screen pixels. Zoom levels above the source's level 14 crop the source vector tile into the requested level, through level 18, rather than enlarging its labels.

## Bounds and cache

- Encoded/decompressed tile input and generated SVG output are limited to 2 MiB each.
- The MVT reader limits layers, features, tag counts, geometry words, integer length and coordinates. SVG output contains numeric paths, escaped titles and local colors; no external fonts or images are referenced.
- At most 24 city/town labels are drawn per tile, with bounded label lengths and overlap checks.
- The themed disk cache is versioned separately from the old raster cache. Source PBFs, rendered SVGs and metadata share one budget of 512 entries and 48,000,000 bytes (48 MB). Source metadata is cached for one day; rendered tiles and vector source tiles are refreshed after seven days. Cached rendered tiles remain usable offline.
- The existing loader retains at most 128 decoded images, with at most four fetch/decode jobs and its two-entry request queue. Themed images rasterize at 512 × 512; retained pixel mip chains can use about 171 MiB per loader. It fetches the visible viewport only. Theme fetch and disk-budget work is serialized across loaders, so several Dashboard maps cannot exceed the provider's parallel request bound. The new theme adds no region download or offscreen prefetch.
- `MOSS_MAP_TILE_URL` keeps the previous custom HTTPS PNG-provider behavior and cache paths.

The public service permits use in apps without an API key, including commercial use. Its [current terms](https://openfreemap.org/tos/) limit an offline-area download to 3,000 tiles or 50 MB, at most four parallel requests, and an app-specific User-Agent. Moss uses one worker per loader, sends an app-specific User-Agent, and keeps its theme cache below that size. The map shows OpenStreetMap and OpenMapTiles attribution, as required by the [OpenFreeMap attribution guide](https://openfreemap.org/#attribution) and [OpenMapTiles schema](https://openmaptiles.org/docs/schema/).

CARTO Voyager was considered as a raster approximation. The [current CARTO terms](https://carto.com/legal/basemap-terms/index.html) require an issued API key, so the app does not use its anonymous CDN.

## Validation

Focused tests cover feature colors, native glyph outlines, SVG decoding and image identity, malformed MVT input, overzoom coordinates, source metadata validation, offline tiles and cache bounds. A separate public-fixture diagnostic renders downloaded OpenFreeMap features; no private map data is uploaded. Native physical trackpad and frame-rate checks remain separate from this theme work.

The public fixture probe decoded nine Perth tiles (1.2 MB of PBF input) and rasterized the themed output successfully. Final cached light/1× and dark/2× map captures under `/tmp/moss-map-theme-review/` show nine visible tiles. The first short headless run finished its simulated frames before the asynchronous SVG rasterizers completed; the settled captures use 30 seconds of virtual time. This is a capture-settling choice, not a native latency measurement.

An independent run under `/tmp/moss-map-theme-live/` started with an empty tile cache and the same generic Perth sample. It fetched live TileJSON and PBF data, styled and cached it, and produced `live.png` with nine visible tiles and no source/tile errors. That check verifies the default provider path; the synthetic and cached checks alone do not.
