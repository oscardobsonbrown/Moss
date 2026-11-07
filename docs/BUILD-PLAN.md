# Build plan

Status: implementation has started. Keep the full agreed v1 scope. Release-detail questions are deferred.

## Build order

1. Run a real Rune app with the workspace shell, persistent pages, editable blocks, local search, and the five sample workspaces. Verify edits and reopen behavior before extending it.
2. Add the database engine: properties, relations, rollups, formulas, filters, sorting, grouping, and templates. Connect real data to table, board, calendar, timeline, gallery, list, chart, feed, map, and dashboard views.
3. Complete workspace windows, tabs, local people, comments, Trash, file attachments and previews, import, and export.
4. Add local buttons and automations across workspaces, reminders, the overdue summary, and optional login startup.
5. Complete component-state comparisons, keyboard and accessibility checks, data-integrity tests, and measured performance checks at the agreed scale.

## Implementation rules

- Use pinned Rune source and Rust. Use SQLite for durable local data. No server is needed to open or edit content.
- Use the existing reference library and record deliberate differences. Missing reference recordings or paid-plan states remain open evidence gaps.
- Build one working path through storage, state, UI, and verification at a time. Do not present a static mockup as a working feature.
- Keep window rendering independent from background work. Ordinary content has an 8.8 ms target, the single-million-row database has a 20 ms target, and cold start has a 300 ms target. Do not claim those results before measurement.
- Run one build at a time with two Cargo jobs on this Mac. Coding agents use separate worktrees and do not start parallel builds.
- Keep private Notion captures out of Git and release artifacts.

## Release gates

All five acceptance workspaces must work. The full retained feature and state inventory, import/export round trips, save/recovery behavior, accessibility, and performance results must be recorded. Remaining gaps must stay explicit.
