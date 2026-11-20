use moss_core::Block;
use rune::{
    TextEdit,
    edit::{Block as Paragraph, Mark, MarkRange},
};
use serde_json::{Value, json};

// Store exact text and byte ranges. HTML alone changes leading and repeated spaces.
pub fn load(block: &Block) -> TextEdit {
    let Some(marks) = block.config.get("marks").and_then(Value::as_array) else {
        let mut edit = block
            .config
            .get("html")
            .and_then(Value::as_str)
            .map(TextEdit::from_html)
            .unwrap_or_else(|| TextEdit::new(&block.text).rich());
        if edit.text != block.text {
            edit.replace_text(&block.text);
        }
        return edit;
    };
    let marks = marks
        .iter()
        .filter_map(|v| {
            let start = usize::try_from(v["start"].as_u64()?).ok()?;
            let end = usize::try_from(v["end"].as_u64()?).ok()?;
            if start >= end
                || end > block.text.len()
                || !block.text.is_char_boundary(start)
                || !block.text.is_char_boundary(end)
            {
                return None;
            }
            let mark = match v["kind"].as_str()? {
                "bold" => Mark::Bold,
                "italic" => Mark::Italic,
                "underline" => Mark::Underline,
                "strike" => Mark::Strike,
                "code" => Mark::Code,
                "link" => Mark::Link(v["value"].as_str()?.to_owned()),
                "pill" => Mark::Pill(v["value"].as_str()?.to_owned()),
                _ => return None,
            };
            Some(MarkRange::new(start..end, mark))
        })
        .collect();
    let paragraphs = block
        .config
        .get("paragraphs")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|v| match v["kind"].as_str() {
                    Some("bullet") => {
                        Paragraph::Bullet(v["level"].as_u64().unwrap_or(0).min(3) as u8)
                    }
                    Some("numbered") => {
                        Paragraph::Numbered(v["level"].as_u64().unwrap_or(0).min(3) as u8)
                    }
                    Some("quote") => Paragraph::Quote,
                    Some("code") => Paragraph::Code,
                    _ => Paragraph::Paragraph,
                })
                .collect()
        })
        .unwrap_or_default();
    let soft = block
        .config
        .get("soft_breaks")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| u32::try_from(v.as_u64()?).ok())
                .collect()
        })
        .unwrap_or_default();
    TextEdit::from_parts(&block.text, marks, paragraphs).with_soft_breaks(soft)
}
pub fn save(block: &mut Block, edit: &TextEdit) {
    crate::text_colors::sync(block, edit);
    block.text = edit.text.clone();
    block.config["html"] = json!(edit.to_html());
    block.config["marks"] = Value::Array(
        edit.marks()
            .iter()
            .filter_map(|m| {
                let (kind, value) = match &m.mark {
                    Mark::Bold => ("bold", None),
                    Mark::Italic => ("italic", None),
                    Mark::Underline => ("underline", None),
                    Mark::Strike => ("strike", None),
                    Mark::Code => ("code", None),
                    Mark::Link(s) => ("link", Some(s)),
                    Mark::Pill(s) => ("pill", Some(s)),
                    _ => return None,
                };
                Some(json!({"start":m.start,"end":m.end,"kind":kind,"value":value}))
            })
            .collect(),
    );
    block.config["paragraphs"] = Value::Array(
        edit.blocks()
            .iter()
            .map(|p| match p {
                Paragraph::Bullet(n) => json!({"kind":"bullet","level":n}),
                Paragraph::Numbered(n) => json!({"kind":"numbered","level":n}),
                Paragraph::Quote => json!({"kind":"quote"}),
                Paragraph::Code => json!({"kind":"code"}),
                Paragraph::Paragraph => json!({"kind":"paragraph"}),
            })
            .collect(),
    );
    block.config["soft_breaks"] = json!(edit.soft_breaks());
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_text_and_marks_survive_a_round_trip() {
        let mut edit = TextEdit::new("  hello   world\n").rich();
        edit.set_selection(2, 7);
        edit.toggle_mark(Mark::Bold);
        let mut block = Block::paragraph("page", "", 0);
        save(&mut block, &edit);
        let loaded = load(&block);
        assert_eq!(loaded.text, edit.text);
        assert_eq!(loaded.marks(), edit.marks());
    }
}
