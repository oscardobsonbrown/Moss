# Acceptance workspaces

Status: the user selected all five scenarios. The fixture details below are a working test specification. No app tests have run.

Use the [video references](../research/notion/WORKFLOW-VIDEOS.md) for workflow ideas. Build these workspaces from blank pages with sample data. They are Rotion test workspaces, not requests to create new Notion workspaces or buy templates.

## Common checks

For each workspace:

1. Create and edit its pages, databases, properties, relations, and views through the UI.
2. Change a record in one view and check the same record in other views. Values must agree.
3. Find a page or record with search. Current-workspace search must exclude other workspaces. All workspaces search must identify each result's workspace.
4. Reopen saved local content with no connection. Network work must not block it.
5. Close its window and reopen it. Check the saved tab, scroll, selection, and workspace rules where applicable.
6. Delete and restore a sample page or record through Trash. Check its content and relationships after restore.
7. Export a Rotion archive and open it as a separate workspace. Compare content, attachments, properties, formulas, relations, views, templates, and stored automation configuration.
8. Inspect Markdown/CSV, HTML, and PDF exports and the report of any losses.
9. Check light and dark themes, keyboard focus, keyboard commands, and the selected pointer-animation rules.
10. Set a date reminder, close all Rotion windows, and keep the service running. Check that a local macOS notification arrives at the configured time when notifications are allowed.
11. Resume after several reminder times and automation times were missed. Check that Rotion shows one overdue-reminder summary and does not run missed automation occurrences.
12. Use a controlled test clock to check Trash retention. Deleted pages and records must remain restorable before the 30-day deadline and expire after that deadline. Do not advance the Mac's clock or use personal data for this test.

Measure ordinary first-viewport readiness against 8.8 ms, including disk reads. Use a separate scale fixture with 100,000 pages and one database of 1 million rows; that database's first-viewport limit is 20 ms. Cold start has the separate 300 ms limit. Use the full [performance contract](PERFORMANCE.md). A workflow demonstration is not a timing result.

## W01: Projects and tasks

Setup: two projects, six tasks, sample notes, and two people from the local People list. Give tasks a project relation, status, due date, and responsibility. Add an ordinary Role property where useful.

Actions and expected results:

- Create tasks from a template. The new task must contain the template's configured structure and values.
- Use table, board, calendar, and timeline views of the same tasks. A change to status or date must appear in each relevant view.
- Complete a task. Project rollups and the configured progress formula must reflect the changed record.
- Inspect a subtask and a task dependency. Follow the specified Notion behavior; detailed dependency cases remain part of the reference study.
- Run a local button and a scheduled local action. Check the created or changed records and the no-catch-up schedule rule.

## W02: School planning

Setup: two courses, four assignments, and one notes page per course. Include due dates, status, a course relation, and a file attachment.

Actions and expected results:

- Build an overview using nested pages and linked database views.
- Open a course and use its filtered assignment view. Other courses' assignments must stay outside that filter.
- Edit a due date and check the calendar and overview.
- Write formatted notes, add a local attachment, and add a comment thread. Reply, resolve, and reopen the thread.
- Open the notes and attachment offline. Saved media previews must show real content and provide ready controls.

## W03: Content production

Setup: six content records with platform, status, planned date, script text, and sample local media. Use Idea, Draft, Ready, and Published as sample status values.

Actions and expected results:

- Build a content board, publishing calendar, gallery, and table from one source database.
- Move a record to another board group and change its planned calendar date. Check the same record in the other views.
- Create a record from a script template and edit its checklist and long-form text.
- Filter by platform and sort by planned date. The visible records must match the selected rules.
- Mark a record Published as a local status change. The fixture does not require an external publishing action.

## W04: Sales CRM

Setup: two companies, three contacts, and four deals. Link contacts and deals to companies. Add a local person responsible for a record, an ordinary Role property, deal value, stage, and next-action date.

Actions and expected results:

- Open a company and find its related contacts and deals.
- Change a deal's stage in a pipeline board. Check its table and company page.
- Calculate the value of won deals using the configured formula or rollup. Check the result against the sample records.
- Change the person responsible and the Role property. This must not create accounts, invitations, or permissions.
- Use a filtered next-action view and open the relevant record notes.

## W05: Meals and groceries

Setup: three recipes, five ingredients, recipe-ingredient records with amounts, and a two-day meal plan. Use a consistent unit for each ingredient. Add sample recipe images and preparation text.

Actions and expected results:

- Browse recipes in a gallery and open a recipe's text and related ingredients.
- Add a recipe to a meal plan without copying its source recipe record.
- Build a shopping view from the selected meals and their related ingredient amounts. Compare its totals with the sample input.
- Change a recipe amount or planned meal and check the shopping result.
- Reopen the recipe and its saved image preview offline.

This fixture tests relations, formulas, and views. It does not require a nutrition service or a new unit-conversion subsystem.

## Additional coverage

These five workflows do not replace the full state matrix or tests for every retained database view. Dashboard, Map, Feed, full property editing, error handling, accessibility, data integrity, and the remaining editor behaviors still need their own cases. Reference recordings and some Notion states remain unavailable or untested.
