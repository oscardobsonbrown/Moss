# Sidebar and page personalization reference

The local desktop study used Notion 7.42.3 on macOS. The computer-use host later failed to start, so the current pass did not verify new native interactions. Private study captures stay under the ignored local research directory.

The saved captures show a sidebar customization mode with Favourites, Recents and Private sections, plus controls to add a tab, shortcut or section and finish editing (capture `004`). Capture `006` shows a searchable Add section chooser. In captures `009` and `010`, accessibility data exposes “Change page icon” on sidebar items and “Add icon” on the page. Those captures do not establish the icon picker’s appearance, choice behavior or persistence. The saved state matrix leaves sidebar row, disclosure, hover and focus states unverified.

Notion’s [page styling guide](https://www.notion.com/help/customize-and-style-your-content) documents emoji, built-in icons and uploaded images, plus removal of an icon. Its [sidebar guide](https://www.notion.com/help/navigate-with-the-sidebar) documents Favorites, nested pages and database view shortcuts. The [workspace guide](https://www.notion.com/help/intro-to-workspaces) documents page-row hover controls and actions including rename, delete, duplicate and move. These web guides can describe a newer product version; they are not measurements of the saved 7.42.3 captures.

Moss’s current tree uses bounded sibling paging and workspace-scoped expansion. Its 30 pt row uses a 26 pt highlight, a 20 pt icon lane, an 8 pt label gap and 16 pt nesting steps. These are Moss implementation dimensions, not measured Notion values. The design should keep the icon lane and title baseline aligned in nested, hovered and selected rows, including long titles and both themes. Paging and disclosure placement are not yet source-verified.

Favorites and an emoji-only page icon picker are implemented locally. The picker includes 3,781 fully-qualified Unicode Emoji 16.0 choices, loads no network data, and mounts at most 64 choice cells. Recent choices last for the current session. Favorites keep up to 100 page IDs per local workspace; missing or trashed pages are hidden and can return after restore. Built-in icon catalogs, image or URL icon uploads, custom workspace emoji, Favorites reordering, database-view shortcuts, page move and duplicate are not established by the local picker captures and remain separate work. Page-icon writes should preserve concurrent page title, parent and settings changes; visible sidebar, tab and record caches should refresh after a successful save. Do not infer a Notion picker layout or interaction result from the AX labels alone.


## Verification on 2026-11-08

The desktop host was retried after implementation and again returned `Sky Computer Use native pipe startup failed` before it reached Notion. No new native Notion behavior was verified in this pass.

The Rune runtime tests exercise an inactive sidebar icon without navigation, emoji search and save, stale-write failure and recovery, all catalog sequences, removal, page/tab/Favorites/child-link/record cache updates, draft preservation, row menu actions, subpage creation, saved Favorites and failed preference writes. Geometry checks cover light and dark themes at 1× and 2×. Picker checks cover the minimum 850×600 window and an eight-column grid with bounded choice cells.

Synthetic-data renderer captures are in `/tmp/moss-sidebar-review-20261108/`. They contain no private Notion content. This is visual and interaction evidence for Moss; it does not establish exact Notion pixels or native presentation performance.
