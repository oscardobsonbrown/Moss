use skrifa::{
    FontRef, MetadataProvider,
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
};
use std::fmt::Write;

struct Pen {
    path: String,
    x: f32,
    y: f32,
}
impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.path, "M{:.2},{:.2}", self.x + x, self.y - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.path, "L{:.2},{:.2}", self.x + x, self.y - y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let _ = write!(
            self.path,
            "Q{:.2},{:.2} {:.2},{:.2}",
            self.x + cx,
            self.y - cy,
            self.x + x,
            self.y - y
        );
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        let _ = write!(
            self.path,
            "C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}",
            self.x + ax,
            self.y - ay,
            self.x + bx,
            self.y - by,
            self.x + x,
            self.y - y
        );
    }
    fn close(&mut self) {
        self.path.push('Z');
    }
}
pub fn path(name: &str, x: f32, y: f32, size: f32) -> Option<(String, f32)> {
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return None;
    }
    let font = FontRef::new(crate::fonts::SANS).ok()?;
    let map = font.charmap();
    let glyphs = name
        .chars()
        .map(|c| map.map(c))
        .collect::<Option<Vec<_>>>()?;
    let metrics = font.glyph_metrics(Size::new(size), LocationRef::default());
    let width = glyphs
        .iter()
        .map(|id| metrics.advance_width(*id).unwrap_or(0.))
        .sum::<f32>();
    let mut pen = Pen {
        path: String::new(),
        x: x - width / 2.,
        y: y + size * 0.35,
    };
    let outlines = font.outline_glyphs();
    for id in glyphs {
        if let Some(outline) = outlines.get(id) {
            outline
                .draw(
                    DrawSettings::unhinted(Size::new(size), LocationRef::default()),
                    &mut pen,
                )
                .ok()?;
        }
        pen.x += metrics.advance_width(id).unwrap_or(0.);
        if pen.path.len() > 128_000 {
            return None;
        }
    }
    Some((pen.path, width))
}
pub fn escape(name: &str) -> String {
    name.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
