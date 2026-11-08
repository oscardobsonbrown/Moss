# Bundled and locally installed fonts

Bundled Geist Sans, Mono and fallback Serif come from [vercel/geist-font](https://github.com/vercel/geist-font), revision `ef2597cfc42f4797676df105517ccc0a1cfa6569`. Keep their OFL.txt and LICENSE.txt notices. Moss embeds these files for offline use.

Page Serif mode can use four locally installed Sentient OTF faces in `~/Library/Fonts`, named `Sentient-Regular.otf`, `Sentient-Bold.otf`, `Sentient-Italic.otf` and `Sentient-BoldItalic.otf`. Moss reads them once at launch. If any face is absent, Serif mode uses the bundled Geist Serif fallback. Geist Sans remains the UI font and Geist Mono remains the code font. Saved Serif page settings need no migration.

Sentient is a proprietary font from [Fontshare](https://www.fontshare.com/fonts/sentient), under the ITF Free Font License. It is not included in Moss or licensed under MIT. A user must obtain their own copy and review the license. Its terms restrict redistribution and making the font available to third parties as a selectable font. This optional local path does not grant redistribution rights. The original license for this installation is in `~/Library/Application Support/Moss/Font licenses/Sentient-FFL.txt`.

`sources.json` records sources and hashes. Exact Notion pixel/weight matching remains unverified.
