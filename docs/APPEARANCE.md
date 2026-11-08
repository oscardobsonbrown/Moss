# Page typography and layout

Source: the user’s screenshot and direct inspection of the Notion desktop app on 2026-11-07. The task-owned Moss UI Study page was used, and its original appearance settings were restored after the comparison.

## Observed behavior

- Actions opens the page menu. Appearance & layout opens a submenu beside it.
- Font style has Default, Serif and Mono, with an “Ag” sample in each font.
- Selecting a font updates the page immediately and keeps the submenu open.
- In the 7.42.3 deep pass, Serif and Mono visibly apply to the page title, body, database view and column labels, row values, status text and New button. The sidebar, window tab and breadcrumb remain sans serif (b-062–b-063). Mono values wrap and increase row height in that fixture. These are visual observations, not font identification or measurements.
- With Small text on, the title, heading and body appear smaller. Table text appears similar in size in that comparison (b-064); no pixel measurement was made. An earlier observation stated that database text also became smaller. Resolve that difference with matched captures and measurements before specifying database scaling. The fixture returned to Default with Small text off (b-065).
- The earlier inspection reported that Full width expands the content area and both switches update without closing the menu. Full width was not changed in the deep pass; its geometry remains to be measured.

## Moss behavior

Interface text uses a shared 14 pt size, including secondary labels and database controls. Page titles, document text, headings, emoji, chart values, and font previews keep their content-specific sizes. Page Small text settings do not change the shared interface labels.

Use **… → Appearance & layout**. The three previews use Geist Sans, locally installed Sentient (or bundled Geist Serif when absent) and Geist Mono. The current Moss design specifies a 14/16 Small text ratio for page content; this is provisional and is not a measured Notion ratio. Database text scaling remains open pending matched measurements. Full width removes the document width cap; the normal document content width remains 720 points within its padded column. Code blocks and inline code keep Geist Mono under every page font.

Store these options in the page’s settings, not global preferences. Defaults are Default, normal text and normal width. Old pages without settings use those defaults. Saved options survive navigation, Quit/reopen and archive round trips. Changing one option preserves the others. Failed saves leave the previous option selected and show an error in the menu.

Geist Sans, Mono and the fallback Serif are embedded from the [official Geist repository](https://github.com/vercel/geist-font). Serif pages use locally installed [Sentient](https://www.fontshare.com/fonts/sentient) when all four regular, bold, italic and bold italic OTF faces are available. Moss reads them once at launch. Sentient is proprietary under the ITF Free Font License and is not distributed with Moss. Each user must obtain their own copy and review its terms. Exact Notion weight and pixel matching remains unverified.

The wider Actions menu and the remaining appearance controls (table of contents, discussions and inline comments) remain part of the full product build. They are not represented by inactive controls in this font change.


## Interface icon family

Use Phosphor Regular SVGs from the shared `ui::phosphor` catalog. Keep the existing 16/18 pt artwork sizes and 20 pt icon lanes where those controls use them. Each control supplies the theme color. Unicode page emoji, keyboard shortcut labels and font-style samples remain text. Source metadata and the MIT notice are under `crates/moss-app/assets/phosphor`.

## Page header and record panel controls

| Before | After | Why |
| --- | --- | --- |
| Add icon used smaller faint text and a different height | Add icon and Add cover use the shared 30 pt text button, with 14 pt text, 9 pt side padding and common hover/pressed states | Match other buttons in full pages and record panels |
| The record side panel had a border on all four edges | Only a 1 pt left divider remains | Remove the top border requested by the user |
| The panel header showed a generic Record label | Open as page and Close remain in the header | Remove the redundant label |
