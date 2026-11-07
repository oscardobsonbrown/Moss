# Rotion instructions

Use ASD-STE100 Simplified Technical English.

## Product records

Read `docs/DECISIONS.md`, `docs/PRD.md`, and `docs/OPEN-QUESTIONS.md` before implementation work. The user has instructed us to start building from the agreed requirements. Continue implementation while tracking reference gaps and deferred release choices. Do not restart the product interview for routine technical choices.

- Use AnomalyCo's Rune UI framework. Do not replace it with a web UI framework.
- Target macOS first.
- V1 is the full personal workspace engine. Do not reduce it to pages or an unapproved subset of database views. The user explicitly excluded Form views from v1.
- V1 is local-only, with no account or sync. Use an app-managed local database and attachment folder, with import and export.
- Use the new Notion desktop tab model: one workspace per window, with a stable sidebar and per-tab scroll and selection. Multiple workspaces can stay open in separate windows.
- Run scheduled automations in every local workspace while Rotion's service runs, even if no window is open for that workspace. Explicit Quit stops all work. Skip missed occurrences.
- Match exact Notion pixels and behavior for retained features. Record deliberate differences, including removed product surfaces and almost no motion.
- Animate pointer-triggered menus, dialogs, and tabs. Keep keyboard-triggered actions and all database view switches instant. Scrolling, dragging, and standard caret feedback remain allowed. Read the PRD for timing decisions; do not infer them from a default preset.
- For clicked tabs, show content immediately and animate only the active-tab indicator. Keyboard tab switches are fully instant.
- Support light and dark themes. Follow the system theme by default.
- Background network fetches must never block local work.
- Keep ordinary content open below 8.8 ms, including disk reads. Test 100,000 pages and one database containing 1 million rows. The first-viewport limit for that large-database case is 20 ms, including disk reads. The first real viewport must be ready for input. Remote embeds and map tiles are exempt. Do not call a target achieved without measurements.

## UI study

The raw captures in `research/notion/captures/` can contain private workspace content. Keep them local. Do not upload them to Paper or publish them. Use task samples for new study material. Do not change existing personal pages.

Give only one agent control of the native Notion app at a time. Use computer use for UI actions. Keep evidence, reproduction steps, context, and gaps together. A still image does not establish motion timing or performance.

The current screenshot library is incomplete. Do not replace untested states with assumed behavior. Read `research/notion/COVERAGE.md` and `STATE-MATRIX.md`.

The Paper project is linked in `README.md`. Its research-board style is not the final app design system.
