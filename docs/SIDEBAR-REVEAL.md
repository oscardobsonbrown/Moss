# Sidebar rail and temporary reveal

The collapsed state uses a 44 pt icon rail below a 40 pt top bar. It keeps loaded page and Favorites icons and 20 × 1 pt rules between groups. Hovering the rail reveals the full sidebar over the content; it does not reflow the page. Moving from the rail into the panel keeps it open. After the pointer leaves, a 120 ms grace period allows the user to return. A sidebar popup must remain usable when it extends outside the panel. Escape and the keyboard pin control need explicit focus and close behavior. The stored collapsed state and expanded width must not change during a temporary reveal.

The local Notion 7.42.3 study includes a collapsed sidebar capture (`OBS-01-09`) and the sidebar customization states `004`–`007`. These do not establish the rail dimensions, reveal delay, popup interaction, keyboard behavior or pixel geometry. The requested design and the app's synthetic tests are therefore implementation requirements, not verified Notion parity. The ChatGPT comparison was blocked: the Sky native pipe and browser host failed after reset and direct app selection. No dimensions were measured from that reference.

All 10 focused sidebar reveal tests pass. The workspace suite passes 314 tests (230 app, 71 core and 13 engine), with two performance probes ignored; the debug app build also passes. Acceptance checks cover rail-to-panel travel, leaving and returning before the grace deadline, eventual close, popup interaction outside the panel, Escape and keyboard pinning, saved preferences across reopen, resize at a narrow window, and database side peek. Check light and dark themes at 1× and 2×. A reveal must not move or resize the content, tab strip or peek. The former New page row below Search is removed; the header compose control remains the creation entry.

## Matching top and left edges — 2026-11-08

| Before | After | Why |
| --- | --- | --- |
| The top bar was 44 pt high and the collapsed rail was 56 pt wide | The top bar is 40 pt high and the rail is 44 pt wide | Use the requested top and left spacing |
| Native titlebar setup, reveal bounds and panel offsets used separate numbers | They use the matching height or width constant | Keep pointer bounds and overlays aligned with the visible edges |

All 56 app tests matching `sidebar_` pass. The existing layout checks now assert equal top and left dimensions and content origin at 850/1440 pt widths, in light/dark themes at 1×/2×. Icon sizes and the saved expanded sidebar width are unchanged.

Light/1× and dark/2× renders under `/tmp/moss-even-chrome-review/` were inspected. Build, formatting and diff checks pass. The development app was restarted. Native physical input was not checked in this sizing pass.

The user then reduced the shared size to 40 pt. Header buttons and the divider are centered from that size, and rail padding keeps its icons clear. All 56 sidebar tests passed; after the padding adjustment, the ten reveal tests passed again. Collapsed and expanded light renders at 40 pt were inspected. Build, formatting and diff checks pass, and the development app was restarted.

The user then set only the collapsed rail back to 44 pt. The top bar remains 40 pt. Rail layout and hover bounds share the 44 pt width; icon slots keep the same inner width. The existing geometry checks assert these separate dimensions.
