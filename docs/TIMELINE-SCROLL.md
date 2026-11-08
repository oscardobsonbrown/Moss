# Timeline fixed column — 2026-11-08

The Name column keeps its horizontal position while the date ruler, grid and bars scroll behind it. The name cells have an opaque surface and a thin right boundary. They block clicks and drags into the hidden grid. The Name heading and date ruler stay at the top during vertical scrolling. Names and bars use one scroll viewport, so their rows cannot drift apart.

| Before | After | Why |
| --- | --- | --- |
| Names and dates moved sideways together | Name cells stay at the left edge | Keep record names available while reading later dates |
| Moving content could show through a fixed cell | Opaque name cells cover the moving grid and block its input | Prevent hidden bars and date-creation targets from receiving input |
| The date ruler moved above the viewport | The ruler stays at the top | Keep dates visible while reading lower rows |
| Period controls and New page could move out of reach sideways | Both keep their horizontal position | Preserve access after scrolling |

The [video review](../research/notion/TIMELINE-SCROLL-REVIEW-20261108.md) covers three public walkthroughs and current Notion Help. They show the separate left table and date region. The scene reports do not prove scroll coupling, sticky-header timing or frame rates. The fixed column follows the user's explicit requirement.

Regression checks cover horizontal and vertical movement, date/bar alignment, page opening from a fixed cell, a hidden grid drag, and view menus above the ruler. Geometry checks use 850 × 600 light/1× and 1100 × 800 dark/2× windows. Existing date-range clipping and date-creation tests remain in place.

The rendered review found that raised row cells could cover the top Name heading. Row cells now use local sticky order; only the date-axis header is raised. A regression checks that a hidden row cannot receive a click through the heading. Final light/1× and dark/2× scrolled renders were inspected under `/tmp/moss-timeline-scroll-review`. All five focused Timeline tests and the 388-test workspace suite pass; two performance probes remain ignored.

This change retains the current 30-day range and loaded record page. It does not establish full Timeline parity, dependency editing, infinite date scrolling or native frame-rate targets. Native computer use remains blocked by the Sky pipe startup error.
