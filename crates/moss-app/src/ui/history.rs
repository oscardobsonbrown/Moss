use super::theme::{Theme, button};
use crate::state::State;
use moss_core::history::HistoryEntry;
use rune::*;
use serde_json::Value;

fn summary(entry: &HistoryEntry) -> &'static str {
    match (
        entry.entity_kind.as_str(),
        entry.before.is_some(),
        entry.after.is_some(),
    ) {
        ("block", false, true) => "Added a block",
        ("block", true, false) => "Removed a block",
        ("block", _, _) => "Changed a block",
        ("record", _, _) => "Changed a property",
        ("record_title", _, _) => "Renamed a record",
        ("record_created", _, _) => "Created a record",
        ("record_duplicated", _, _) => "Duplicated a record",
        ("record_trashed", _, _) => "Moved a record to Trash",
        ("record_restored", _, _) => "Restored a record",
        ("relation", _, _) => "Changed a relation",
        ("page", _, _) => "Changed page title",
        _ => "Changed content",
    }
}

fn content(value: Option<&Value>) -> String {
    match value {
        None => "No content".into(),
        Some(Value::Null) => "Empty".into(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(map)) if map.get("text").and_then(Value::as_str).is_some() => {
            map["text"].as_str().unwrap_or("").to_owned()
        }
        Some(value) => value.to_string(),
    }
}

pub fn panel(s: &State, t: Theme, width: f32) -> Option<El> {
    if !s.history.open
        || !(s
            .tab()
            .is_some_and(|tab| Some(&tab.page.id) == s.history.page_id.as_ref())
            || s.record_peek
                .as_ref()
                .is_some_and(|peek| Some(&peek.page.id) == s.history.page_id.as_ref()))
    {
        return None;
    }
    let list = s.history.entries.iter().map(|entry| {
        let sequence = entry.sequence;
        let actor = entry
            .actor
            .as_ref()
            .map_or("Unknown", |actor| actor.name.as_str());
        let agent = entry
            .actor
            .as_ref()
            .is_some_and(|actor| actor.kind == "agent");
        let when = chrono::DateTime::parse_from_rfc3339(&entry.occurred_at)
            .map(|time| {
                time.with_timezone(&chrono::Local)
                    .format("%d %b %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|_| entry.occurred_at.clone());
        let icon = if agent {
            rune::icon(&super::phosphor::HEXAGON_FILL, 18.).color(t.blue)
        } else {
            el().size(16., 16.).radius(8.).fill(t.blue)
        };
        row()
            .key(format!("history-entry-{sequence}"))
            .w_full()
            .padding(8.)
            .gap(8.)
            .center_y()
            .role("button")
            .label(format!("{actor}: {}", summary(entry)))
            .focusable(true)
            .radius(5.)
            .hover(t.hover)
            .fill(
                if s.history
                    .selected
                    .as_ref()
                    .is_some_and(|selected| selected.sequence == sequence)
                {
                    t.selected
                } else {
                    t.bg
                },
            )
            .child(icon)
            .child(
                col()
                    .grow(1.)
                    .min_w(0.)
                    .child(
                        t.label(actor, t.text)
                            .font(TextStyle::new(14., 600.).family(t.family)),
                    )
                    .child(t.label(summary(entry), t.muted)),
            )
            .child(t.sized_label(when, 12., t.muted))
            .on_press(move |s: &mut State| s.select_history(sequence))
    });
    let details = s.history.selected.as_ref().map(|entry| {
        let restore = matches!(entry.entity_kind.as_str(), "block" | "record");
        let earlier = content(entry.before.as_ref());
        col()
            .w_full()
            .gap(8.)
            .padding(10.)
            .border(1., t.line)
            .radius(6.)
            .child(t.label("Before", t.muted))
            .child(t.label(content(entry.before.as_ref()), t.text).wrap())
            .child(t.label("After", t.muted))
            .child(t.label(content(entry.after.as_ref()), t.text).wrap())
            .children(entry.before.is_some().then(|| {
                button("history-copy", "Copy earlier content", t)
                    .on_press_tx(move |_: &mut State, tx| tx.copy(earlier.clone()))
            }))
            .children(restore.then(|| {
                button("history-restore", "Restore earlier content", t)
                    .on_press(|s: &mut State| s.restore_history())
            }))
    });
    Some(
        col()
            .key("history-panel")
            .w(width)
            .min_w(width)
            .h_full()
            .fill(t.bg)
            .child(
                el().w(1.)
                    .h_full()
                    .pin(Some(0.), Some(0.), None, Some(0.))
                    .fill(t.line)
                    .inert(),
            )
            .child(
                row()
                    .w_full()
                    .h(42.)
                    .px(14.)
                    .center_y()
                    .child(
                        t.label("History", t.text)
                            .font(TextStyle::new(16., 600.).family(t.family))
                            .grow(1.),
                    )
                    .child(
                        button("history-close", "Close", t)
                            .on_press(|s: &mut State| s.history.open = false),
                    ),
            )
            .child(
                row()
                    .w_full()
                    .px(12.)
                    .gap(4.)
                    .child(
                        button("history-tab", "History", t)
                            .fill(if s.history.live { t.bg } else { t.selected })
                            .on_press(|s: &mut State| s.set_history_live(false)),
                    )
                    .child(
                        button("history-live", "Live", t)
                            .fill(if s.history.live { t.selected } else { t.bg })
                            .on_press(|s: &mut State| s.set_history_live(true)),
                    ),
            )
            .child(
                col()
                    .w_full()
                    .grow(1.)
                    .min_h(0.)
                    .padding(12.)
                    .gap(8.)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .children(
                        s.history
                            .entries
                            .is_empty()
                            .then(|| t.label("No recorded changes yet.", t.muted)),
                    )
                    .children(list)
                    .children((s.history.has_more && !s.history.live).then(|| {
                        button("history-more", "Load more", t)
                            .on_press(|s: &mut State| s.load_more_history())
                    }))
                    .children(details)
                    .children(
                        s.history
                            .error
                            .as_ref()
                            .map(|error| t.label(error, t.error).wrap()),
                    ),
            ),
    )
}
