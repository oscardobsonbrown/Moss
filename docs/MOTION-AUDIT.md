# Motion audit

Status: in progress. Rune is pinned to `f3d04f3`; these are implementation observations and headless runtime checks, not measured Notion timings. The user requires pointer-triggered menus and dialogs to animate, keyboard-triggered equivalents to be instant, and database view switches to be instant. A pointer tab switch must show its content immediately and animate only its active indicator. See [PRD](PRD.md) and [decisions](DECISIONS.md).

## Current implementation policy

This is a provisional engineering policy, not a claim about Notion motion. Menu and dialog presence uses an in-place opacity transition. Rune's current `Transition::opacity()` uses its `SETTLE` spring pair (0.30-second enter and 0.24-second exit spring parameters, zero bounce); those parameters are not a fixed completion time. The active tab indicator uses an in-place opacity spring with a 0.15-second parameter and zero bounce. Content, database view changes, input, hover, drag and scrolling receive no added motion from this change. Keyboard actions use `Tx::instant()` for their commit. That snaps changes caused by the key but does not by itself settle unrelated tracks already running.

Under system Reduce Motion, Rune continues to use an in-place fade for presence; it suppresses position effects. A headless Search check confirms an in-place fade with no rectangle movement. Reduced-motion duration and indicator behavior need a separate product decision and wider tests. Rapid interruption and focus return need checks for each surface; an Escape and immediate keyboard reopen of Search are tested below.

## Evidence

The pinned Rune `Tx::instant` documentation states that it snaps layout and suppresses mount/unmount transitions. The same `on_press` handler can be reached by pointer or keyboard. Rune's event route invokes the focused element's ancestor `on_key` handlers before keyboard activation; Moss's root key handler calls `tx.instant()` and returns false. Headless tests check the actual outcome, rather than treating a handler name as proof of input origin.

| Target | Observed result and remaining work |
| --- | --- |
| Search dialog | Pointer opacity starts below 1 and increases; Cmd+K opens at 1. Escape then keyboard reopen lands immediately. |
| Database Views chooser | Before the fix, a pointer opened it at opacity 1 because its handler forced `tx.instant`. After removing that force, pointer opacity increases over time; a keyboard Space activation opens at 1. The same pointer and keyboard checks pass after the chooser was anchored. |
| Table Status picker | Pointer open shows an increasing opacity after the Table handler stopped forcing instant. Keyboard Table activation is instant in a headless test. A pointer option selection fades the picker out. Test date, select, other close actions and failed-save paths separately. |
| Database view switch | The headless pointer test sees the Board control at full opacity in the first frame. Keyboard view switching and other layouts need separate checks. |
| Desktop tab | The headless pointer test sees the new document at full opacity while the active indicator is still fading; keyboard activation lands both at full opacity. Other tab operations and interruption need tests. |
| Gallery Layout and Calendar/Timeline No date | These anchored panels were inside the instant content subtree. They are now mounted at the root overlay layer. Headless pointer tests show increasing opacity for Gallery Layout and Calendar No date; keyboard Space opens each at full opacity. Timeline, focus return and dismissal still need separate tests. |
| Record peeks and other menus | Pointer-open handlers were forcing instant in several view paths. A headless Gallery New test sees the centre peek fade while database content remains fully visible; pointer Close fades the peek out. Other enter/exit and focus paths need separate tests. |

The test module is `crates/moss-app/src/motion_tests.rs`. It uses temporary workspaces and Rune's headless `presented` geometry and effective opacity, not the user's app data. It does not measure rendered pixels, frame timing on hardware, Notion motion, or every interaction.
