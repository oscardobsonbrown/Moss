use std::sync::OnceLock;

pub struct Emoji {
    pub glyph: &'static str,
    pub name: &'static str,
    pub category: &'static str,
}
pub const PAGE_SIZE: usize = 64;
pub const CATEGORIES: &[&str] = &[
    "Smileys & Emotion",
    "People & Body",
    "Animals & Nature",
    "Food & Drink",
    "Travel & Places",
    "Activities",
    "Objects",
    "Symbols",
    "Flags",
];
fn catalog() -> &'static [Emoji] {
    static CATALOG: OnceLock<Vec<Emoji>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        include_str!("../assets/emoji/unicode-16.tsv")
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                Some(Emoji {
                    category: parts.next()?,
                    glyph: parts.next()?,
                    name: parts.next()?,
                })
            })
            .collect()
    })
}
pub fn matches(query: &str, category: Option<&str>) -> Vec<&'static Emoji> {
    let query = query.trim().to_lowercase();
    catalog()
        .iter()
        .filter(|e| category.is_none_or(|category| e.category == category))
        .filter(|e| {
            query.is_empty()
                || e.name.contains(&query)
                || e.category.to_lowercase().contains(&query)
                || e.glyph.contains(&query)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_finds_named_categories_and_multi_codepoint_emoji() {
        assert!(
            matches("technologist", None)
                .iter()
                .any(|e| e.glyph == "🧑‍💻")
        );
        assert!(matches("australia", None).iter().any(|e| e.glyph == "🇦🇺"));
        assert!(matches("1️⃣", None).iter().any(|e| e.glyph == "1️⃣"));
        assert!(matches("no such emoji", None).is_empty());
        assert_eq!(matches("", None).len(), 3781);
    }
}
