use moss_core::{PageFont, PageSettings};
use rune::{Family, TextStyle};

pub const SANS: &[u8] = include_bytes!("../assets/fonts/Geist.ttf");
pub const MONO: &[u8] = include_bytes!("../assets/fonts/GeistMono.ttf");
pub const SERIF: &[u8] = include_bytes!("../assets/fonts/GeistSerif.ttf");
pub const FONTS: &[&[u8]] = &[
    SANS,
    include_bytes!("../assets/fonts/Geist-Italic.ttf"),
    MONO,
    include_bytes!("../assets/fonts/GeistMono-Italic.ttf"),
    SERIF,
];
pub const FAMILIES: (&str, &str) = ("Geist", "Geist Mono");

struct FontSet {
    bytes: Vec<&'static [u8]>,
    family: &'static str,
    #[cfg(test)]
    serif_faces: Vec<&'static [u8]>,
}
fn font_set() -> &'static FontSet {
    static SET: std::sync::OnceLock<FontSet> = std::sync::OnceLock::new();
    SET.get_or_init(|| {
        let folder = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join("Library/Fonts");
        let names = ["Regular", "Bold", "Italic", "BoldItalic"];
        let installed: std::io::Result<Vec<Vec<u8>>> = names
            .iter()
            .map(|name| std::fs::read(folder.join(format!("Sentient-{name}.otf"))))
            .collect();
        let mut bytes = FONTS.to_vec();
        match installed {
            Ok(files) => {
                let faces: Vec<&'static [u8]> = files
                    .into_iter()
                    .map(|file| &*Box::leak(file.into_boxed_slice()))
                    .collect();
                bytes.extend(faces.iter().copied());
                FontSet {
                    bytes,
                    family: "Sentient",
                    #[cfg(test)]
                    serif_faces: faces,
                }
            }
            Err(_) => FontSet {
                bytes,
                family: "Geist Serif",
                #[cfg(test)]
                serif_faces: vec![SERIF],
            },
        }
    })
}
/// Load the optional locally installed serif once, keeping page opens free of font file reads.
pub fn all() -> &'static [&'static [u8]] {
    &font_set().bytes
}

pub fn family(font: PageFont) -> Family {
    match font {
        PageFont::Default => Family::Sans,
        PageFont::Serif => Family::Named(font_set().family),
        PageFont::Mono => Family::Mono,
    }
}
pub fn scale(settings: PageSettings) -> f32 {
    if settings.small_text { 0.875 } else { 1. }
}
pub fn page_style(settings: PageSettings, size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size * scale(settings), weight).family(family(settings.font))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renderer_uses_the_bundled_fonts_instead_of_fallbacks() {
        let mut system = rune::text::TextSystem::new(all(), FAMILIES);
        for (font, bytes) in [
            (PageFont::Default, SANS),
            (PageFont::Serif, font_set().serif_faces[0]),
            (PageFont::Mono, MONO),
        ] {
            let layout = system.layout_rich(
                "Ag Reading 0123",
                &[],
                page_style(
                    PageSettings {
                        font,
                        ..Default::default()
                    },
                    16.,
                    400.,
                ),
                1.,
                None,
            );
            let mut count = 0;
            for line in layout.lines() {
                for run in line.runs() {
                    assert_eq!(run.font().data.as_ref(), bytes, "wrong font for {font:?}");
                    count += 1;
                }
            }
            assert!(count > 0);
        }
        let serif = page_style(
            PageSettings {
                font: PageFont::Serif,
                ..Default::default()
            },
            16.,
            400.,
        );
        let inline = system.layout_rich(
            "code",
            &[rune::Span {
                start: 0,
                end: 4,
                mono: true,
                ..Default::default()
            }],
            serif,
            1.,
            None,
        );
        let block = system.layout_rich("code", &[], serif.mono(), 1., None);
        for layout in [inline, block] {
            for line in layout.lines() {
                for run in line.runs() {
                    assert_eq!(
                        run.font().data.as_ref(),
                        MONO,
                        "code must use Geist Mono even on a Serif page"
                    );
                }
            }
        }
    }
    #[test]
    fn installed_serif_uses_real_bold_and_italic_faces() {
        let faces = &font_set().serif_faces;
        if faces.len() != 4 {
            return;
        }
        let mut system = rune::text::TextSystem::new(all(), FAMILIES);
        for (weight, italic, expected) in [
            (400., false, 0),
            (700., false, 1),
            (400., true, 2),
            (700., true, 3),
        ] {
            let style = TextStyle::new(16., weight).family(family(PageFont::Serif));
            let style = if italic { style.italic() } else { style };
            let layout = system.layout_rich("Ag Reading", &[], style, 1., None);
            for line in layout.lines() {
                for run in line.runs() {
                    assert_eq!(
                        run.font().data.as_ref(),
                        faces[expected],
                        "wrong serif style"
                    );
                }
            }
        }
    }
}
