use super::theme::{Theme, button};
use crate::state::{State, Tab};
use rune::*;

use super::phosphor::FILE_TEXT as PAGE;

pub fn view(tab: &Tab, t: Theme) -> Option<El> {
    let Some(children) = tab.child_pages.as_ref() else {
        if !matches!(
            tab.page.kind,
            moss_core::PageKind::Document | moss_core::PageKind::Record
        ) {
            return None;
        }
        let id = tab.page.id.clone();
        return Some(
            row()
                .key("child-pages-retry")
                .gap(8.)
                .center_y()
                .child(t.label("Could not load child pages.", t.muted))
                .child(
                    button("child-pages-retry-button", "Retry", t)
                        .on_press(move |s: &mut State| s.refresh_child_pages(&id)),
                ),
        );
    };
    if children.items.is_empty() && children.previous.is_none() && children.next.is_none() {
        return None;
    }
    let mut list = col().key("child-pages").w_full().gap(2.).pt(8.);
    for child in &children.items {
        if tab.page_links.contains_key(&child.page.id) {
            continue;
        }
        let id = child.page.id.clone();
        let title = if child.page.title.is_empty() {
            "Untitled"
        } else {
            &child.page.title
        };
        list = list.child(
            row()
                .key(format!("child-page-{id}"))
                .w_full()
                .h(32.)
                .px(6.)
                .gap(8.)
                .center_y()
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label(title)
                .focusable(true)
                .child(
                    el().size(20., 20.)
                        .center()
                        .child(if child.page.icon.is_empty() {
                            rune::icon(&PAGE, 16.).color(t.muted)
                        } else {
                            t.sized_label(&child.page.icon, 16., t.text)
                        }),
                )
                .child(t.sized_label(title, 15., t.text).truncate().min_w(0.))
                .on_press_tx(move |s: &mut State, tx| {
                    s.open_page(&id, false);
                    tx.instant();
                }),
        );
    }
    let mut paging = row().gap(4.);
    if let Some(cursor) = &children.previous {
        let cursor = cursor.clone();
        paging = paging.child(button("child-pages-previous", "Previous", t).on_press_tx(
            move |s: &mut State, tx| {
                s.page_children_move(cursor.clone(), false);
                tx.instant();
            },
        ));
    }
    if let Some(cursor) = &children.next {
        let cursor = cursor.clone();
        paging = paging.child(button("child-pages-next", "Next", t).on_press_tx(
            move |s: &mut State, tx| {
                s.page_children_move(cursor.clone(), true);
                tx.instant();
            },
        ));
    }
    Some(list.child(paging))
}
