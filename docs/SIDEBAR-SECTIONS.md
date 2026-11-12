# Sidebar sections reference — 2026-11-08

The current task adds local, per-workspace shortcut sections. A user can create, rename and remove a section, reorder sections by drag or menu, and collapse them. Recent shows four unique actual page opens, including opens through tabs, history and peeks; startup restoration does not count as an open. Favorites supports pages and subpages. Page actions use the requested trailing placement with a 3 pt offset. These are Moss requirements; they do not establish exact Notion behavior or geometry.

[Notion: Navigate with the sidebar](https://www.notion.com/help/navigate-with-the-sidebar) documents dragging sections, collapsing section headings, moving sections up or down from their menus, Favorites, and nested page toggles. [Notion: Customize your sidebar](https://www.notion.com/help/customize-your-sidebar) describes shortcuts and section ordering within sidebar tabs, and says hiding a shortcut or section does not delete its content. That tab-based model does not prove the behavior of Moss custom sections. The documents were consulted on 2026-11-08.

The [community video](https://www.youtube.com/watch?v=87YLOpV1kCU) and [Notion's linked video](https://www.youtube.com/watch?v=lSmgY5OsZmU) were attempted, but the web fetch was throttled. No frames or transcript were obtained, so no video behavior is claimed. The native Notion host also failed at the Sky native pipe. The exact menu labels, drag feedback, hover states, recent-page policy, shortcut removal behavior and pixel dimensions remain reference gaps.

Validation covers persistence and workspace isolation, section actions and ordering, collapse, recent deduplication and startup behavior, nested Favorites, trailing action placement, persistent input focus and caret tracking, and stale-target refresh.

The workspace suite passed 333 tests (249 app, 71 core and 13 engine), with two performance probes ignored. The app build, formatting and diff checks passed. Current light and dark synthetic renders were inspected. Synthetic light full-sidebar, dark section-menu and collapsed-rail renders were inspected in `/tmp/moss-sidebar-sections-20261108/`.

Recent resolves at most five pages for an update and does not reload the custom-section cache on a visit. Preferences are written synchronously. No open-time performance measurement was made for this path.

The sidebar uses one index-built virtual row list for section headers, Private, Favorites, Recent, custom shortcuts and New section in each sidebar state. This avoids a growing virtual Private list followed by sibling sections, which could overlap in Rune. Tests confirm that expanded Private and a following custom section do not overlap. With 1,200 shortcuts, fewer than 100 shortcut rows mount at either end, and the last shortcut remains reachable.

Pages can be dragged between Private, Favorites and custom sections. Custom sections hold shortcuts, so a page keeps its place in the Private tree. Moving a shortcut to Private removes the shortcut; moving a favorite to Private removes the favorite. Dropping on a collapsed section opens it. Recent pages cannot be dragged or used as a drop target.

Pages in Private can also be dragged to a new position among their siblings. The order is stored with each page and works across the paged sidebar list. This does not change the page's parent.

The collapsed and revealed sidebar use the same surface, rows and scroll view. Hover changes the width of that surface without moving the page content. The compact state shows the page icons and section dividers; the revealed state shows their labels and controls.
