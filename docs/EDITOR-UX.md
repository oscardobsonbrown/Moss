# Editor alignment and interaction reference

The supplied user screenshots establish the requested layout direction: the page icon, title, paragraphs and plain headings share a leading content edge; list markers and callout content have intentional insets. The screenshot is a design reference, not a matched native measurement. Gutter controls must remain reachable as the pointer moves from text to the controls, without dropping the text selection or focus. The plus and grip controls use separate 20 × 24 pt targets.

Notion 7.42.3 local captures b-024–b-028 show toggle creation, disclosure and a new sibling after Return. Captures b-029–b-031 show a selection toolbar, a bold-toggle change and a link picker. Captures b-032–b-034 show a bordered callout, a new text block inside it on Return, and an empty paragraph outside after two Returns. These are individual observed states and actions, not full editor coverage. Capture b-035 shows unmatched slash text retained with no visible menu in that image. Private reference images remain in the ignored research directory; native capture is currently unavailable because the computer-use host does not start.

Captures b-062–b-063 show Serif and Mono on page content while the sidebar, tab and breadcrumb remain sans. Small text appears to shrink page title, heading and body in b-064, but has not been measured. Moss uses Geist Sans, locally installed Sentient with Geist Serif fallback, and Geist Mono; the font system is a deliberate difference. Exact colors, dimensions and animation timing remain unverified against matched native images.

The alignment regression uses a synthetic document, two window widths, both themes, page font modes, Full width on and off, and 1×/2× scale. It checks shared leading edges for the icon control, title, paragraphs and headings, with explicit marker and callout insets. It also checks gutter hit targets and the move from the focused text to plus and grip controls. These checks protect Moss geometry and interaction; they do not claim pixel parity with Notion.

Open gaps include native measurements for gutter geometry and hover/focus transitions, full callout nesting and exit behavior, slash-search and no-results states, and block conversion/keyboard behavior across all block types. Keep those gaps distinct from the synthetic checks and from the user-provided design direction.


## Implemented in this pass

The empty-block “Press / to insert blocks” hint appears only on block hover or focus. It is removed when the input contains text. The hint uses the block's content font and does not change the input hit target or caret.

The page icon control, title, paragraphs, plain headings and Divider share one content edge. The icon is no longer centered in an oversized horizontal box. Intentional list, code and callout insets remain. A 50 pt gutter holds separate 20 × 24 pt insert and action controls; the content has enough margin to keep both targets inside the page at the minimum window width. The standalone footer plus is replaced by a blank writing area that focuses or creates a final empty paragraph without splitting existing text.

The `/`, gutter plus and grip open an anchored, searchable block menu with Phosphor icons, a Basic blocks label, heading shortcut hints, keyboard selection and Escape. Only supported block types are listed. Slash cancellation restores literal `/query` text. Code blocks retain literal slashes. The grip also exposes Duplicate and Delete. Closing a menu restores input focus and selection. Adding to an empty paragraph reuses that line. A Divider leaves an editable paragraph after it; choosing it for a nonempty block inserts below that text rather than hiding it.

Enter keeps the existing rich-text split and list continuation, while Enter on an empty list returns to a paragraph. Typed-space shortcuts cover headings, bullets, numbers, to-dos, toggles and quotes. Shift-Enter remains a soft line break. Empty-block Backspace removes a preceding Divider or exits the current structured type without focusing an invisible input. Completed to-do styling does not carry into other block types. Failed command saves keep the draft, offer Retry and Keep draft and close, and retain the unsaved-page guard.

The documented keyboard sources are the official [Notion shortcut guide](https://www.notion.com/help/keyboard-shortcuts) and [page styling guide](https://www.notion.com/help/customize-and-style-your-content). In particular, `>` then Space creates a Toggle, and `"` then Space creates a Quote. The local `---` then Space shortcut is not established by the supplied captures as exact Notion behavior.

## Remaining limits

This pass does not implement full block nesting, drag reordering, multi-block selection, a selection-formatting toolbar, structural undo across block splits, or every Notion block type. Core block saves still replace the page body without a cross-window expected-version check; concurrent writers for the same page need a separate store-level conflict guard. Native matched-pixel and motion validation remains blocked: the desktop host was retried and again returned `Sky Computer Use native pipe startup failed`.
