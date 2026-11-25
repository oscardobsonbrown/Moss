use crate::state::State;
use std::{
    ops::Range,
    path::{Component, Path, PathBuf},
};

/// Byte ranges retain the exact Markdown source so editing never rewrites imported notes.
pub fn links(text: &str) -> Vec<(Range<usize>, String)> {
    if !text.contains('[') {
        return Vec::new();
    }
    let bytes = text.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    fn escaped(bytes: &[u8], pos: usize) -> bool {
        let mut n = 0;
        let mut i = pos;
        while i > 0 && bytes[i - 1] == b'\\' {
            i -= 1;
            n += 1;
        }
        n % 2 != 0
    }
    while i < bytes.len() {
        if bytes[i] == b'`' && !escaped(bytes, i) {
            let start = i;
            while i < bytes.len() && bytes[i] == b'`' {
                i += 1;
            }
            let delimiter = &text[start..i];
            if let Some(end) = text[i..].find(delimiter) {
                i += end + delimiter.len();
            }
            continue;
        }
        if bytes[i] != b'['
            || escaped(bytes, i)
            || (i > 0 && bytes[i - 1] == b'!' && !escaped(bytes, i - 1))
        {
            i += 1;
            continue;
        }
        if bytes.get(i + 1) == Some(&b'[') {
            let mut end = i + 2;
            while end + 1 < bytes.len()
                && !(bytes[end] == b']' && bytes[end + 1] == b']' && !escaped(bytes, end))
            {
                end += 1;
            }
            if end + 1 < bytes.len() {
                let inner = &text[i + 2..end];
                let (target, label) = inner.split_once('|').unwrap_or((inner, inner));
                if !target.is_empty() && !label.is_empty() {
                    result.push((end - label.len()..end, format!("moss-wiki:{target}")));
                }
                i = end + 2;
                continue;
            }
        } else {
            let mut end = i + 1;
            let mut depth = 0usize;
            while end < bytes.len() {
                if bytes[end] == b'\n' {
                    break;
                }
                if !escaped(bytes, end) {
                    match bytes[end] {
                        b'[' => depth += 1,
                        b']' if depth > 0 => depth -= 1,
                        b']' => break,
                        _ => {}
                    }
                }
                end += 1;
            }
            if end + 1 < bytes.len() && bytes[end] == b']' && bytes[end + 1] == b'(' {
                let mut close = end + 2;
                let mut depth = 0usize;
                while close < bytes.len() {
                    if bytes[close] == b'\n' {
                        break;
                    }
                    if !escaped(bytes, close) {
                        match bytes[close] {
                            b'(' => depth += 1,
                            b')' if depth > 0 => depth -= 1,
                            b')' => break,
                            _ => {}
                        }
                    }
                    close += 1;
                }
                if close < bytes.len() && bytes[close] == b')' && end > i + 1 && close > end + 2 {
                    result.push((i + 1..end, text[end + 2..close].to_owned()));
                    i = close + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    result
}

pub fn hidden_markdown_syntax(
    edit: &rune::TextEdit,
    links: &[(Range<usize>, String)],
) -> Vec<rune::Span> {
    let mut spans = Vec::new();
    for (label, target) in links {
        let Some(start) = label.start.checked_sub(1) else {
            continue;
        };
        let Some(end) = label.end.checked_add(target.len() + 3) else {
            continue;
        };
        if end > edit.text.len()
            || edit.text.as_bytes()[start] != b'['
            || edit.text.as_bytes().get(label.end..label.end + 2) != Some(b"](")
            || edit.text.as_bytes()[end - 1] != b')'
        {
            continue;
        }
        let a = edit.anchor.min(edit.focus);
        let b = edit.anchor.max(edit.focus);
        if edit.focused && a <= end && b >= start {
            continue;
        }
        for range in [start..label.start, label.end..end] {
            spans.push(
                rune::Span::new(range.start, range.end)
                    .size(0.)
                    .color(rune::Color::TRANSPARENT),
            );
        }
    }
    spans
}
fn unescape_url(value: &str) -> String {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push((a * 16 + b) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn resolve_path(source: &str, target: &str) -> String {
    let parent = Path::new(source).parent().unwrap_or(Path::new(""));
    let mut path = PathBuf::new();
    for part in parent.join(target).components() {
        match part {
            Component::Normal(value) => path.push(value),
            Component::ParentDir => {
                path.pop();
            }
            Component::CurDir => {}
            _ => return String::new(),
        }
    }
    path.to_string_lossy().into_owned()
}
fn heading_key(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}
impl State {
    fn open_link_page(&mut self, id: &str, anchor: Option<&str>, tx: &mut rune::Tx) {
        if !self.tab().is_some_and(|tab| tab.page.id == id) {
            self.open_page(id, false);
        }
        if let Some(anchor) = anchor.filter(|a| !a.is_empty()) {
            let key = heading_key(&unescape_url(anchor));
            let target = self
                .tab()
                .filter(|tab| tab.page.id == id)
                .and_then(|tab| {
                    tab.blocks.iter().find(|block| {
                        matches!(
                            block.block.kind,
                            moss_core::BlockKind::Heading1
                                | moss_core::BlockKind::Heading2
                                | moss_core::BlockKind::Heading3
                        ) && heading_key(&block.edit.text) == key
                    })
                })
                .map(|block| block.block.id.clone());
            if let Some(id) = target {
                tx.scroll_into_view(format!("block-{id}"));
            } else {
                self.error = Some("The linked heading is not available.".into());
            }
        }
    }
    pub fn open_inline_link(&mut self, value: &str, tx: &mut rune::Tx) {
        let source = self
            .tab()
            .and_then(|tab| {
                tab.blocks.iter().find_map(|block| {
                    block
                        .block
                        .config
                        .get("import_source_path")
                        .and_then(serde_json::Value::as_str)
                })
            })
            .unwrap_or("")
            .to_owned();
        let page = self
            .tab()
            .map(|tab| tab.page.id.clone())
            .unwrap_or_default();
        self.open_inline_link_from(value, &source, &page, tx);
    }
    pub fn open_peek_inline_link(&mut self, value: &str, tx: &mut rune::Tx) {
        let Some(peek) = &self.record_peek else {
            return;
        };
        let source = peek
            .blocks
            .iter()
            .find_map(|block| {
                block
                    .block
                    .config
                    .get("import_source_path")
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or("")
            .to_owned();
        let page = peek.page.id.clone();
        if !value.starts_with("https://")
            && !value.starts_with("http://")
            && !value.starts_with("mailto:")
            && !self.close_peek()
        {
            return;
        }
        self.open_inline_link_from(value, &source, &page, tx);
    }
    fn open_inline_link_from(
        &mut self,
        value: &str,
        source: &str,
        source_page: &str,
        tx: &mut rune::Tx,
    ) {
        if value.starts_with("https://")
            || value.starts_with("http://")
            || value.starts_with("mailto:")
        {
            if let Err(error) = tx.open_url(value) {
                self.error = Some(error.to_string());
            }
            return;
        }
        if let Some(id) = value.strip_prefix("moss-page:") {
            match self.store.get_page(id) {
                Ok(Some(page))
                    if page.workspace_id == self.workspace.id && page.deleted_at.is_none() =>
                {
                    self.open_link_page(id, None, tx)
                }
                _ => self.error = Some("Linked page is not available.".into()),
            }
            return;
        }
        let wiki = value.strip_prefix("moss-wiki:");
        let target = wiki.unwrap_or(value);
        let (raw, anchor) = target
            .split_once('#')
            .map_or((target, None), |(path, anchor)| (path, Some(anchor)));
        if raw.is_empty() {
            if !source_page.is_empty() {
                self.open_link_page(source_page, anchor, tx);
            }
            return;
        }
        let mut path = unescape_url(raw);
        if wiki.is_some() && !path.ends_with(".md") {
            path.push_str(".md");
        }
        let resolved = resolve_path(source, &path);
        if let Ok(Some(page)) = self
            .store
            .page_for_import_path(&self.workspace.id, &resolved)
        {
            self.open_link_page(&page.id, anchor, tx);
            return;
        }
        if wiki.is_some() && path != resolved {
            if let Ok(Some(page)) = self.store.page_for_import_path(&self.workspace.id, &path) {
                self.open_link_page(&page.id, anchor, tx);
                return;
            }
        }
        let name = Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let name = name.strip_suffix("_all").unwrap_or(name);
        let name = name
            .rsplit_once(' ')
            .filter(|(_, id)| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
            .map_or(name, |(title, _)| title);
        match self.store.page_for_unique_title(&self.workspace.id, name) {
            Ok(Some(page)) => self.open_link_page(&page.id, anchor, tx),
            Ok(None) => {
                self.error = Some("Linked page is missing or its name is ambiguous.".into())
            }
            Err(error) => self.error = Some(format!("Link could not be resolved: {error}")),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markdown_syntax_is_hidden_except_at_the_cursor_or_selection() {
        let text = "Start [One](https://one.test/a_(b)) and [Two](other.md) [[Wiki]] end";
        let mut edit = rune::TextEdit::new(text);
        let links = links(text);
        let ranges = |edit: &rune::TextEdit| {
            hidden_markdown_syntax(edit, &links)
                .into_iter()
                .map(|s| s.start as usize..s.end as usize)
                .collect::<Vec<_>>()
        };
        let hidden = ranges(&edit);
        assert_eq!(hidden.len(), 4);
        assert_eq!(
            hidden.iter().map(|r| &text[r.clone()]).collect::<Vec<_>>(),
            vec!["[", "](https://one.test/a_(b))", "[", "](other.md)"]
        );
        edit.focused = true;
        edit.anchor = text.find("One").unwrap();
        edit.focus = edit.anchor;
        assert_eq!(ranges(&edit), hidden[2..]);
        edit.anchor = text.find(" and ").unwrap();
        edit.focus = edit.anchor;
        assert_eq!(ranges(&edit), hidden[2..]);
        edit.anchor = 0;
        edit.focus = text.len();
        assert!(ranges(&edit).is_empty());
        assert_eq!(edit.text, text);
    }
    #[test]
    fn hidden_links_take_only_label_space_and_still_open() {
        use rune::{Env, Runtime};
        let path = std::env::temp_dir().join(format!("moss-markdown-link-{}", moss_core::new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let source = state.tab().unwrap().page.id.clone();
        let target = state
            .store
            .create_page(
                &state.workspace.id,
                None,
                "Target",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        let destination = format!("moss-page:{}", target.id);
        let text =
            format!("See [First]({destination}) and [Second](https://example.test/a-long-url) end");
        state.tabs[state.active].blocks[0].edit.replace_text(&text);
        state.save_blocks();
        let mut rt = Runtime::with_text(
            state,
            crate::ui::view,
            (1000., 800.),
            Env::default(),
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.frame(0.);
        rt.frame(0.02);
        let first = rt.find(&destination).expect("first link");
        let second = rt
            .find("https://example.test/a-long-url")
            .expect("second link");
        assert!(
            (second.0 - first.0) < 100.,
            "hidden URL still takes space: {first:?} {second:?}"
        );
        rt.pointer_down(first.0, first.1);
        rt.pointer_up(first.0, first.1);
        rt.frame(0.02);
        assert_eq!(rt.state.tab().unwrap().page.id, target.id);
        assert_eq!(rt.state.store.get_blocks(&source).unwrap()[0].text, text);
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn clicking_a_wiki_link_in_an_editable_block_opens_its_page() {
        use rune::{Env, Runtime};
        let path = std::env::temp_dir().join(format!("moss-inline-link-{}", moss_core::new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let target = state
            .store
            .create_page(
                &state.workspace.id,
                None,
                "Unique target",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("See [[Unique target]]");
        state.save_blocks();
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.frame(0.);
        let (x, y) = rt
            .find("moss-wiki:Unique target")
            .expect("wiki link must be accessible");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 60.);
        assert_eq!(
            rt.state.tab().unwrap().page.id,
            target.id,
            "error: {:?}",
            rt.state.error
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn imported_relative_path_wins_over_duplicate_titles() {
        use rune::{Env, Runtime};
        let path = std::env::temp_dir().join(format!("moss-imported-link-{}", moss_core::new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        state.new_page(false, None);
        let target = state
            .store
            .create_page(
                &state.workspace.id,
                None,
                "Target",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        let duplicate = state
            .store
            .create_page(
                &state.workspace.id,
                None,
                "Target",
                "",
                moss_core::PageKind::Document,
            )
            .unwrap();
        let mut target_block = moss_core::Block::paragraph(&target.id, "Target text", 0);
        target_block.config = serde_json::json!({"import_source_path":"Notes/Target.md"});
        state
            .store
            .save_blocks(&target.id, &[target_block])
            .unwrap();
        let mut duplicate_block = moss_core::Block::paragraph(&duplicate.id, "Other", 0);
        duplicate_block.config = serde_json::json!({"import_source_path":"Elsewhere/Target.md"});
        state
            .store
            .save_blocks(&duplicate.id, &[duplicate_block])
            .unwrap();
        state.tabs[state.active].blocks[0].block.config =
            serde_json::json!({"import_source_path":"Notes/Source.md"});
        state.tabs[state.active].blocks[0]
            .edit
            .replace_text("[[Target]]");
        state.save_blocks();
        let mut rt = Runtime::new(state, crate::ui::view, (1000., 800.), Env::default());
        rt.frame(0.);
        let (x, y) = rt.find("moss-wiki:Target").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 60.);
        assert_eq!(rt.state.tab().unwrap().page.id, target.id);
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn respects_escapes_parentheses_code_and_heading_targets() {
        let text = r"\[not a link](https://bad.test) [docs](https://example.com/a_(b)#part) [[Note#Heading|alias]] `[[code]]`";
        let found = links(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, "https://example.com/a_(b)#part");
        assert_eq!(found[1].1, "moss-wiki:Note#Heading");
        assert_eq!(&text[found[1].0.clone()], "alias");
        assert_eq!(heading_key("A Heading!"), "a-heading");
    }
    #[test]
    fn parses_unicode_wiki_markdown_and_ignores_images() {
        let text = "É [[A/B|label]] [Example](https://example.com) ![photo](x.png)";
        let result = links(text);
        assert_eq!(result.len(), 2);
        assert_eq!(&text[result[0].0.clone()], "label");
        assert_eq!(result[0].1, "moss-wiki:A/B");
        assert_eq!(&text[result[1].0.clone()], "Example");
        assert_eq!(
            resolve_path("Area/Note.md", "../Target%20note.md"),
            "Target%20note.md"
        );
        assert_eq!(unescape_url("Target%20note.md"), "Target note.md");
    }
}
