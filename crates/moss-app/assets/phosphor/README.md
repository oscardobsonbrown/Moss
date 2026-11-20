# Phosphor Regular icons

Source: https://github.com/phosphor-icons/core
Revision: `2b75f3ad12b420c9504ef05df8d2564a28f8500e` (package 2.1.1).
Weight: Regular.
License: MIT, reproduced in `LICENSE`.

The SVG files are unmodified upstream assets. `sources.json` records their hashes. Only the icons used by Moss are bundled. Rune renders the 256-unit SVG paths in the control color, with no React, JavaScript, network fetch, or icon font dependency.

The `toolbar` icons use the regular paths with a 4-unit stroke added for toolbar and sidebar actions. Their source paths and hashes are recorded under `regular` in `sources.json`.

`catalog.rs` contains all 1,512 regular icons from the same revision, with the same 4-unit stroke, for the view icon picker and table headers. The catalogue is bundled for offline use under the Phosphor MIT licence.

`fill/hexagon-fill.svg` is the unmodified filled hexagon from the same revision, used for agent avatars.

Use the shared constants in `ui/phosphor.rs` for interface icons. Keep existing control sizes and fixed icon lanes. User-selected Unicode emoji are separate. The File and File Text pair keeps the blank/content page distinction, including the two interior lines in File Text.
