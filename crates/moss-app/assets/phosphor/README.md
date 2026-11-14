# Phosphor Regular icons

Source: https://github.com/phosphor-icons/core
Revision: `2b75f3ad12b420c9504ef05df8d2564a28f8500e` (package 2.1.1).
Weight: Regular.
License: MIT, reproduced in `LICENSE`.

The SVG files are unmodified upstream assets. `sources.json` records their hashes. Only the icons used by Moss are bundled. Rune renders the 256-unit SVG paths in the control color, with no React, JavaScript, network fetch, or icon font dependency.

The `toolbar` icons use the regular paths with a 4-unit stroke added for the database toolbar. Their source paths and hashes are recorded under `regular` in `sources.json`.

Use the shared constants in `ui/phosphor.rs` for interface icons. Keep existing control sizes and fixed icon lanes. User-selected Unicode emoji are separate. The File and File Text pair keeps the blank/content page distinction, including the two interior lines in File Text.
