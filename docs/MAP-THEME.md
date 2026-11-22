# Native map theme

Moss uses an OSM-based interpretation of the requested [Apple Map Style](https://snazzymaps.com/style/241528/apple-map-style) in both light and dark themes.

On macOS arm64, the default renderer is **MapLibre Native (Metal)** via `crates/moss-maplibre`, streaming vector tiles from [OpenFreeMap](https://openfreemap.org/) (`https://tiles.openfreemap.org/planet`). A custom `DirectIOSurfaceResource` renders the visible viewport directly into a 4-slot `IOSurface`-backed `CVPixelBuffer` ring (`MTLPixelFormatBGRA8Unorm`) with non-blocking async Metal commits, and hands completed buffers straight to `rune::NativeSurface` with zero CPU pixel readback and zero intermediate blit passes. Camera movement, pin placement, clustering, and side-peek opening stay unchanged; maps remain pin-less unless the database configures a Place property with location records.

| Before | After | Why |
| --- | --- | --- |
| CPU MVT → SVG → `resvg` tile pipeline | Direct-to-IOSurface MapLibre Native Metal renderer | Smooth 120 Hz-capable vector rendering with zero CPU readback |
| Single light-only tile palette | Embedded 22-layer Light and Dark Moss Apple styles | Follow Moss light/dark/system theme with live style switching |
| External map-style APIs would need a key | OpenFreeMap vector tiles + native Metal rendering | Keep the map usable without an account, paid service, or WebView |

The light palette uses warm land `#f7f1df`, natural areas `#d0e3b4`, parks `#bde6ab`, water `#a2daf2`, medical areas `#fbd3da`, airports `#cfb2db`, yellow highways `#ffe15f` with `#efd151` edges, white arterials, and flat 2D buildings at zoom 14+. The dark palette adapts the same 22-layer structure to `#1f2124` land, `#213426` parks, `#162b3d` water, `#282b30` buildings, `#444952` arterials, and `#b89434` highways. Road labels exclude `rail`, `transit`, `path`, `track`, and `service` classes to avoid station platform clutter.

## Bounds and cache

- MapLibre Native stores its local SQLite ambient cache at `<workspace>/map-tile-cache/maplibre-cache.db` with a 48 MB (`48 * 1024 * 1024` bytes) ceiling, staying below OpenFreeMap's 50 MB offline cache bound. Cached vector tiles and font glyphs remain usable offline.
- MapLibre Native runs on a dedicated worker thread per active map surface (main Map view or Dashboard map widget) and is dropped when the user leaves the view, so background fetches and GPU encoding never block the UI thread.
- `crates/moss-maplibre/build.rs` links the SHA-256-verified precompiled `libmaplibre-native-core-amalgam-macos-arm64-metal.a` archive (`core-a33d3f00fb9dbf9117ce18edd799aa0ca5bcce5b`) with no CMake or Homebrew dylib dependencies.
- `MOSS_MAP_TILE_URL` keeps the previous custom HTTPS PNG-provider behavior and cache paths. `MOSS_MAP_NETWORK=off` disables network map access. Non-macOS targets continue to fall back to `MapTileLoader`.

The public service permits use in apps without an API key, including commercial use. Its [current terms](https://openfreemap.org/tos/) limit an offline-area download to 3,000 tiles or 50 MB, at most four parallel requests, and an app-specific User-Agent. Moss uses one worker per loader, sends an app-specific User-Agent, and keeps its theme cache below that size. The map shows OpenStreetMap and OpenMapTiles attribution, as required by the [OpenFreeMap attribution guide](https://openfreemap.org/#attribution) and [OpenMapTiles schema](https://openmaptiles.org/docs/schema/).

CARTO Voyager was considered as a raster approximation. The [current CARTO terms](https://carto.com/legal/basemap-terms/index.html) require an issued API key, so the app does not use its anonymous CDN.

## Validation

Focused tests cover feature colors, native glyph outlines, SVG decoding and image identity, malformed MVT input, overzoom coordinates, source metadata validation, offline tiles and cache bounds. A separate public-fixture diagnostic renders downloaded OpenFreeMap features; no private map data is uploaded. Native physical trackpad and frame-rate checks remain separate from this theme work.

The public fixture probe decoded nine Perth tiles (1.2 MB of PBF input) and rasterized the themed output successfully. Final cached light/1× and dark/2× map captures under `/tmp/moss-map-theme-review/` show nine visible tiles. The first short headless run finished its simulated frames before the asynchronous SVG rasterizers completed; the settled captures use 30 seconds of virtual time. This is a capture-settling choice, not a native latency measurement.

An independent run under `/tmp/moss-map-theme-live/` started with an empty tile cache and the same generic Perth sample. It fetched live TileJSON and PBF data, styled and cached it, and produced `live.png` with nine visible tiles and no source/tile errors. That check verifies the default provider path; the synthetic and cached checks alone do not.
