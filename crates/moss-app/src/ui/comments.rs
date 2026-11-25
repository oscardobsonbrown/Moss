use super::theme::{Theme, button};
use crate::state::State;
use moss_core::comments::CommentAnchor;
use rune::*;

pub(super) fn agent_avatar(initial: String, size: f32, t: Theme) -> El {
    row()
        .size(size, size)
        .center()
        .child(
            rune::icon(&super::phosphor::HEXAGON_FILL, size - 2.)
                .color(rune::color::oklch(0.58, 0.2, 300.))
                .absolute(1., 1.)
                .inert(),
        )
        .child(t.sized_label(initial, 12., Color::WHITE))
}

pub fn panel(s: &State, t: Theme, width: f32) -> Option<El> {
    if !s.comments.open {
        return None;
    }
    let page_id = s.comments.page_id.as_ref()?;
    let visible = s.tab().is_some_and(|tab| &tab.page.id == page_id)
        || s.record_peek
            .as_ref()
            .is_some_and(|peek| &peek.page.id == page_id);
    if !visible {
        return None;
    }
    let threads = s.comments.threads.get(page_id).into_iter().flatten();
    let items = threads.map(|thread| {
        let id = thread.id.clone();
        let resolved = thread.resolved;
        let messages = thread.messages.iter().map(|message| {
            let initial = message
                .author
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            let avatar = match message.kind.as_deref() {
                Some("person") => row()
                    .size(24., 24.)
                    .center()
                    .radius(12.)
                    .fill(t.blue)
                    .child(t.sized_label(initial, 12., Color::WHITE))
                    .a11y_hidden(),
                Some("agent") => agent_avatar(initial, 24., t).a11y_hidden(),
                _ => el().size(24., 24.).a11y_hidden(),
            };
            let time = chrono::DateTime::parse_from_rfc3339(&message.created_at)
                .map(|date| {
                    date.with_timezone(&chrono::Local)
                        .format("%d %b %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|_| message.created_at.clone());
            col()
                .w_full()
                .gap(4.)
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .gap(6.)
                        .child(avatar)
                        .child(
                            t.label(&message.author, t.text)
                                .font(TextStyle::new(14., 600.))
                                .grow(1.),
                        )
                        .child(t.sized_label(time, 12., t.muted)),
                )
                .child(t.label(&message.text, t.text).wrap())
        });
        let reply = thread.id.clone();
        col()
            .key(format!("comment-thread-{id}"))
            .w_full()
            .padding(10.)
            .gap(8.)
            .border(1., t.line)
            .radius(6.)
            .children(messages)
            .children(
                (!thread.anchor_available)
                    .then(|| t.label("Original content is unavailable.", t.muted).wrap()),
            )
            .child(
                row()
                    .gap(6.)
                    .child(button(format!("comment-reply-{id}"), "Reply", t).on_press(
                        move |s: &mut State| {
                            if !s.comments.input.text.trim().is_empty() {
                                s.comments.error = Some("Finish the current comment first.".into());
                                return;
                            }
                            s.comments.reply = Some(reply.clone());
                            s.comments.anchor = None;
                            s.comments.input.focused = true;
                        },
                    ))
                    .child(
                        button(
                            format!("comment-resolve-{id}"),
                            if resolved { "Reopen" } else { "Resolve" },
                            t,
                        )
                        .on_press(move |s: &mut State| s.comment_resolved(&id, !resolved)),
                    ),
            )
    });
    let adding = s.comments.anchor.is_some() || s.comments.reply.is_some();
    let page = page_id.clone();
    Some(
        col()
            .key("comments-panel")
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
                        t.label("Comments", t.text)
                            .font(TextStyle::new(16., 600.))
                            .grow(1.),
                    )
                    .child(
                        button("comments-close", "Close", t)
                            .on_press(|s: &mut State| s.comments.open = false),
                    ),
            )
            .child(
                col()
                    .w_full()
                    .grow(1.)
                    .min_h(0.)
                    .padding(12.)
                    .gap(10.)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .children(items),
            )
            .children(adding.then(|| {
                col()
                    .w_full()
                    .padding(12.)
                    .gap(8.)
                    .child(
                        super::inputs::field(
                            &s.comments.input,
                            "comment-input",
                            "Write a comment…",
                            t,
                        )
                        .label("Comment text")
                        .on_edit(|s: &mut State| &mut s.comments.input)
                        .on_submit(|s: &mut State, tx| {
                            s.submit_comment();
                            tx.instant();
                        }),
                    )
                    .children(
                        s.comments
                            .error
                            .as_ref()
                            .map(|error| t.label(error, t.error).wrap()),
                    )
                    .child(
                        row()
                            .gap(6.)
                            .child(
                                button("comment-submit", "Post", t)
                                    .on_press(|s: &mut State| s.submit_comment()),
                            )
                            .child(button("comment-cancel", "Cancel", t).on_press(
                                |s: &mut State| {
                                    s.comments.input.clear();
                                    s.comments.input.focused = false;
                                    s.comments.anchor = None;
                                    s.comments.reply = None;
                                    s.comments.error = None;
                                },
                            )),
                    )
            }))
            .children((!adding).then(|| {
                button("comment-new", "Add comment", t).on_press(move |s: &mut State| {
                    s.open_comments(&page, Some(CommentAnchor::Page));
                })
            })),
    )
}

pub fn keyed_ranges(
    s: &State,
    page_id: &str,
    block_id: &str,
    text: &str,
) -> Vec<(String, std::ops::Range<usize>)> {
    s.comments
        .threads
        .get(page_id)
        .into_iter()
        .flatten()
        .filter_map(|thread| {
            if thread.resolved || !thread.anchor_available {
                return None;
            }
            match &thread.anchor {
                CommentAnchor::Text {
                    block_id: id,
                    start,
                    end,
                    quote,
                } if id == block_id && text.get(*start..*end) == Some(quote.as_str()) => {
                    Some((thread.id.clone(), *start..*end))
                }
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
pub fn ranges(s: &State, page_id: &str, block_id: &str, text: &str) -> Vec<std::ops::Range<usize>> {
    keyed_ranges(s, page_id, block_id, text)
        .into_iter()
        .map(|(_, range)| range)
        .collect()
}

pub fn highlight_key(thread: &str, peek: bool) -> String {
    format!(
        "{}comment-highlight-{thread}",
        if peek { "peek-" } else { "" }
    )
}

pub fn page_has_active_comments(s: &State, page_id: &str) -> bool {
    s.comments
        .threads
        .get(page_id)
        .is_some_and(|threads| threads.iter().any(|t| !t.resolved && t.anchor_available))
}

pub fn block_has_active_comments(s: &State, page_id: &str, block_id: &str) -> bool {
    s.comments.threads.get(page_id).is_some_and(|threads| {
        threads.iter().any(|thread| {
            !thread.resolved
                && thread.anchor_available
                && match &thread.anchor {
                    CommentAnchor::Block { block_id: id }
                    | CommentAnchor::Text { block_id: id, .. } => id == block_id,
                    _ => false,
                }
        })
    })
}

pub fn markers(s: &State, page_id: &str, block_id: &str, peek: bool, t: Theme) -> Vec<El> {
    if !block_has_active_comments(s, page_id, block_id) {
        return Vec::new();
    }
    let text = if peek {
        s.record_peek
            .as_ref()
            .filter(|p| p.page.id == page_id)
            .and_then(|p| p.blocks.iter().find(|b| b.block.id == block_id))
            .map(|b| b.edit.text.as_str())
    } else {
        s.tab()
            .filter(|p| p.page.id == page_id)
            .and_then(|p| p.blocks.iter().find(|b| b.block.id == block_id))
            .map(|b| b.edit.text.as_str())
    };
    let prefix = if peek { "peek-" } else { "" };
    let slot = format!("{}editor-block-slot-{block_id}", prefix);
    let slot_y = rune::memo::window_layout_with(&slot, |r| r[1]);
    let mut locations: Vec<(f32, String)> = s
        .comments
        .threads
        .get(page_id)
        .into_iter()
        .flatten()
        .filter_map(|thread| {
            if thread.resolved || !thread.anchor_available {
                return None;
            }
            let top = match &thread.anchor {
                CommentAnchor::Block { block_id: id } if id == block_id => 0.,
                CommentAnchor::Text {
                    block_id: id,
                    start,
                    end,
                    quote,
                } if id == block_id
                    && text.and_then(|text| text.get(*start..*end)) == Some(quote.as_str()) =>
                {
                    let y =
                        rune::memo::window_layout_with(&highlight_key(&thread.id, peek), |r| r[1]);
                    match (y, slot_y) {
                        (Some(y), Some(slot)) => (y - slot).max(0.),
                        _ => 0.,
                    }
                }
                _ => return None,
            };
            Some((top, thread.id.clone()))
        })
        .collect();
    locations.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut groups: Vec<(f32, Vec<String>)> = Vec::new();
    for (top, id) in locations {
        if let Some((last_top, ids)) = groups.last_mut() {
            if top - *last_top < 24. {
                ids.push(id);
                continue;
            }
        }
        groups.push((top, vec![id]));
    }
    groups
        .into_iter()
        .map(|(top, ids)| {
            let page = page_id.to_owned();
            let thread_id = ids[0].clone();
            let count = ids.len();
            let label = if count == 1 {
                "Open comments".to_string()
            } else {
                format!("Open {count} comments")
            };
            let mut marker = row()
                .key(format!("{prefix}comment-marker-{block_id}-{thread_id}"))
                .pin(None, Some(top), Some(-26.), None)
                .size(24., 24.)
                .center()
                .radius(4.)
                .role("button")
                .label(label)
                .focusable(true)
                .hover(t.hover)
                .child(
                    rune::icon(super::view_icons::catalog_icon("chat-circle").unwrap(), 16.)
                        .color(rune::color::oklch(0.68, 0.14, 80.)),
                );
            if count > 1 {
                marker = marker.child(
                    t.sized_label(count.to_string(), 10., t.text)
                        .absolute(15., 0.)
                        .fill(t.bg)
                        .radius(4.),
                );
            }
            marker.on_press_tx(move |s: &mut State, tx| {
                s.open_comments(&page, None);
                tx.scroll_into_view(format!("comment-thread-{thread_id}"));
                tx.instant();
            })
        })
        .collect()
}
