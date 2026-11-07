# Page typography and layout

Source: the user’s screenshot and direct inspection of the Notion desktop app on 2026-11-07. The task-owned Rotion UI Study page was used, and its original appearance settings were restored after the comparison.

## Observed behavior

- Actions opens the page menu. Appearance & layout opens a submenu beside it.
- Font style has Default, Serif and Mono, with an “Ag” sample in each font.
- Selecting a font updates the page immediately and keeps the submenu open.
- The font applies to the page title, headings, body and inline database content. App chrome and menu labels keep the UI font.
- Small text reduces page content sizes, including the title and database text. Full width expands the content area. Both update without closing the menu.

## Rotion behavior

Use **… → Appearance & layout**. The three previews use Geist Sans, Geist Serif and Geist Mono. Small text uses a 14/16 size ratio. Full width removes the document width cap; the normal document content width remains 720 points within its padded column. Code blocks and inline code keep Geist Mono under every page font.

Store these options in the page’s settings, not global preferences. Defaults are Default, normal text and normal width. Old pages without settings use those defaults. Saved options survive navigation, Quit/reopen and archive round trips. Changing one option preserves the others. Failed saves leave the previous option selected and show an error in the menu.

Font files are embedded from the [official Geist repository](https://github.com/vercel/geist-font). The upstream [Geist Serif announcement](https://vercel.com/blog/introducing-geist-serif) describes its current display-face scope. Geist is an explicit visual difference from Notion.

The wider Actions menu and the remaining appearance controls (table of contents, discussions and inline comments) remain part of the full product build. They are not represented by inactive controls in this font change.
