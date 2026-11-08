# Notion documented behavior: Date, Status, and Gallery

Checked 7 November 2026. These are **documented-only** claims from Notion's official help. They are not new native observations and do not prove behavior in the installed 7.42.3 build. Native evidence and its limits remain in `research/notion/DEEP-PASS-B.md`. This research does not count toward the native discovery hour.

## Date and reminders

- A Date property accepts a single date or a date range, with optional time. The timeline guide describes an End date control and an Include time toggle. This supports storing a range and optional time; it does not specify the picker interaction or validation rules. [Database properties](https://www.notion.com/help/database-properties), [Timeline guide](https://www.notion.com/help/guides/timeline-view-unlocks-high-output-planning-for-your-team).
- The database Date calendar can set a reminder, its lead time, a time, date format, and time zone. The help describes Notion notifications, including desktop push and Inbox behavior. This describes the reference service; it does not establish that Moss has a local notification scheduler or require cloud delivery. [Reminders](https://www.notion.com/help/reminders).
- Region affects whether calendar weeks start on Sunday or Monday; Notion's preference can override that setting, including in reminder and calendar date pickers. The preference also supports a manually set or automatically updated time zone. [Account settings](https://www.notion.com/help/account-settings).

## Status and keyboard

- Status options belong to three fixed groups: To-do, In progress, and Complete. The guide describes custom options and colors, and a choice of select-style or checkbox display. Select accepts one option; Multi-select accepts one or more. [Status guide](https://www.notion.com/help/guides/status-property-gives-clarity-on-tasks), [Database properties](https://www.notion.com/help/database-properties).
- The shortcut page documents Mac `Control+Shift+K` and `Control+Shift+J` for the previous and next database page in peek view. It does not specify the Table cell Tab/Shift-Tab sequence, Status picker selection or Escape behavior, or Date input validation. Its Tab and Shift-Tab descriptions for nested text blocks must not be applied to database cells. [Keyboard shortcuts](https://www.notion.com/help/keyboard-shortcuts).

## Gallery

- Gallery layout offers card size and card preview settings. Page cover uses the page cover; Page content previews the first page block, including an image or video when that is the first block. A Files & media property can also provide a preview. [Gallery help](https://www.notion.com/help/galleries).
- Fit image on fits the complete image within the card frame; off crops it to fill the frame. The same help describes dragging cards to reorder them and showing, hiding, and reordering visible properties, including hiding Name. [Gallery help](https://www.notion.com/help/galleries).
- The help does not give card dimensions, responsive breakpoints, a None preview specification, or New page tile geometry. The current Moss `gallery.rs` marks Large geometry provisional and renders a text preview; the official help identifies first-block media and image fitting as behavior to account for, but it does not define an exact implementation. Check the current code and product scope before assigning a change.

## Native probes still needed

1. Repeat title → Tab → Shift-Tab → Return from an isolated known cell. Check selected-cell outline, activation, and AX focus separately.
2. Check Status and Select pointer choice, Escape, outside dismissal, and focus return. Check Date clear, invalid input, range/time changes, reminders, Escape, and outside dismissal on task-owned rows.
3. Capture Gallery Large at known window size and at a narrower size. Compare Page content and None, New page tile, card wrapping, and media fit. Measure geometry only from matching fresh screenshots and AX context.
4. Confirm any local notification behavior and time-zone edge cases in Moss itself. Notion's service notification description is not evidence of offline delivery.

Keep remote integrations, shared access, and external messages outside this local-only reference task. Do not upload raw private captures.

## Sidebar, Library, and nested pages

- The sidebar edge can be dragged to resize it. The collapse control hides or expands the sidebar, and section headings hide or reveal their contents. This does not specify animation, persistence, or disclosure hover behavior. [Navigate with the sidebar](https://www.notion.com/help/navigate-with-the-sidebar).
- The sidebar Home tab has a configurable number of items in each section. Library offers Recents, Favorites, Private and other sections, with page-name search and filters; a section removed from the sidebar remains accessible there. The documentation describes Library access, but does not establish that every `View all` control opens the same Library tab in every installed version. [Navigate with the sidebar](https://www.notion.com/help/navigate-with-the-sidebar), [Manage your Library](https://www.notion.com/help/manage-your-library).
- A subpage is a page within a page. `/page` creates one in the current page, and expanding the parent in the sidebar reveals its subpages. [Create a subpage](https://www.notion.com/help/create-a-subpage).
- Each database item is a page. Its body below the properties can contain blocks, including subpages and inline databases. This supports nested page content in record pages; the docs do not specify Moss's data model or how a record appears in the sidebar. [Intro to databases](https://www.notion.com/help/intro-to-databases?trk=article-ssr-frontend-pulse_little-text-block).

## Date range and time limits

- The Date property's `End date` switch enables a date range; `Include time` permits a specific time. `Clear` removes the property's value. These descriptions do not say whether a switch immediately writes a value, which endpoint receives a calendar click, or what happens on Escape or invalid input. Those behaviors remain native probes, not documented findings. [Database properties](https://www.notion.com/help/database-properties).
- The local task-row AX sequence b-126–129 separately shows a typed date, a later populated cell, and restoration to empty after Clear. Because native input was delayed, it cannot establish the exact commit trigger or timing. It does not answer the range or time questions.
