use super::{icons, theme::Theme};
use crate::state::Tab;
use moss_core::Record;
use rune::*;

pub fn view(tab: &Tab, record: &Record, key: impl Into<String>, t: Theme) -> El {
    let custom = tab
        .record_icons
        .get(&record.id)
        .map(String::as_str)
        .unwrap_or("");
    let has_content = tab
        .record_has_content
        .get(&record.id)
        .copied()
        .unwrap_or(false);
    appearance(custom, has_content, key, t)
}

pub fn appearance(custom: &str, has_content: bool, key: impl Into<String>, t: Theme) -> El {
    let (label, icon) = if custom.is_empty() {
        let icon = if has_content {
            &icons::PAGE_CONTENT
        } else {
            &icons::PAGE_BLANK
        };
        (
            if has_content {
                "Page with content"
            } else {
                "Empty page"
            }
            .to_owned(),
            rune::icon(icon, 16.).color(t.muted),
        )
    } else {
        (
            format!("Page icon {custom}"),
            t.sized_label(custom, 16., t.text),
        )
    };
    row()
        .key(key)
        .size(16., 16.)
        .center()
        .role("img")
        .label(label)
        .child(icon)
}

pub fn title_button(
    tab: &Tab,
    record: &Record,
    index: usize,
    key: String,
    prefix: &str,
    t: Theme,
) -> El {
    let title = if record.title.is_empty() {
        "Untitled"
    } else {
        &record.title
    };
    row()
        .key(key)
        .h(30.)
        .px(9.)
        .gap(6.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(title)
        .focusable(true)
        .child(view(
            tab,
            record,
            format!("record-icon-{prefix}-{index}"),
            t,
        ))
        .child(t.label(title, t.text).min_w(0.).grow(1.).truncate())
        .on_press_tx(move |s: &mut crate::state::State, tx| {
            s.open_record(index);
            tx.instant();
        })
}
