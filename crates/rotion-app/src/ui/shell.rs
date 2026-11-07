use super::theme::{Theme, button, icon_button, label};
use crate::state::{Modal, State};
use rune::*;

pub fn titlebar(s: &State, cx: &mut Cx<State>, t: Theme) -> El {
    let insets = cx.chrome_insets();
    let tabs = s.tabs.iter().enumerate().map(|(i, tab)| {
        let title = if tab.title.text.is_empty() {
            "Untitled"
        } else {
            &tab.title.text
        };
        row()
            .key(format!("tab-{i}"))
            .h(36.)
            .w(174.)
            .min_w(70.)
            .px(10.)
            .gap(7.)
            .center_y()
            .radius(5.)
            .fill(if i == s.active { t.bg } else { t.sidebar })
            .hover(t.hover)
            .child(
                el().key(format!("tab-indicator-{i}"))
                    .h(2.)
                    .pin(Some(8.), None, Some(8.), Some(0.))
                    .fill(t.text)
                    .opacity(if i == s.active { 1. } else { 0. })
                    .opacity_motion(Spring::new(0.15, 0.)),
            )
            .child(
                row()
                    .grow(1.)
                    .min_w(0.)
                    .role("tab")
                    .label(title)
                    .focus_selected(i == s.active)
                    .focusable(true)
                    .child(label(format!("{}  {title}", tab.page.icon), 12., t.text).truncate())
                    .on_press_tx(move |s: &mut State, _| {
                        if !s.flush_current() {
                            return;
                        }
                        s.active = i;
                        s.save_preferences();
                    }),
            )
            .child(
                icon_button(format!("close-tab-{i}"), "Close tab", "×", t)
                    .size(18., 22.)
                    .on_press_tx(move |s: &mut State, tx| {
                        tx.instant();
                        s.close_tab(i);
                    }),
            )
    });
    row()
        .key("titlebar")
        .w_full()
        .h(44.)
        .fill(t.sidebar)
        .window_drag()
        .center_y()
        .gap(3.)
        .pl(insets.leading.max(78.) + 10.)
        .pr(insets.trailing + 8.)
        .children(tabs)
        .child(
            icon_button("new-tab", "New tab", "+", t).on_press(|s: &mut State| {
                s.new_name = TextEdit::new("");
                s.modal = Some(Modal::NewPage);
            }),
        )
        .child(el().grow(1.))
}
fn nav(key: &str, icon: &str, name: &str, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(30.)
        .px(10.)
        .gap(9.)
        .center_y()
        .radius(5.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .focusable(true)
        .child(super::icons::glyph(icon, 18., t.muted).w(20.))
        .child(label(name, 14., t.muted))
}
pub fn sidebar(s: &State, t: Theme) -> El {
    let current = s.tab().map(|t| t.page.id.as_str()).unwrap_or("");
    let page_rows = col()
        .key("page-list")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows::<State>(
            s.pages.len(),
            30.,
            |s, i| std::borrow::Cow::Borrowed(&s.pages[i].id),
            move |s, i| {
                let p = &s.pages[i];
                let id = p.id.clone();
                let active = s.tab().is_some_and(|t| t.page.id == p.id);
                row()
                    .key(format!("page-{}", p.id))
                    .w_full()
                    .h(30.)
                    .px(10.)
                    .gap(8.)
                    .center_y()
                    .radius(4.)
                    .role("treeitem")
                    .label(&p.title)
                    .focus_selected(active)
                    .focusable(true)
                    .fill(if active { t.selected } else { t.sidebar })
                    .hover(t.hover)
                    .child(
                        label(
                            if p.parent_id.is_some() {
                                "  ›"
                            } else {
                                "›"
                            },
                            15.,
                            t.faint,
                        )
                        .w(if p.parent_id.is_some() { 24. } else { 12. }),
                    )
                    .child(label(&p.icon, 16., t.text).w(20.))
                    .child(
                        label(
                            if p.title.is_empty() {
                                "Untitled"
                            } else {
                                &p.title
                            },
                            14.,
                            t.text,
                        )
                        .truncate()
                        .grow(1.)
                        .min_w(0.),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        tx.instant();
                        s.open_page(&id, false);
                    })
            },
        );
    let _ = current;
    col()
        .key("sidebar")
        .w(248.)
        .h_full()
        .fill(t.sidebar)
        .px(8.)
        .gap(2.)
        .child(
            row()
                .key("workspace-picker")
                .h(40.)
                .w_full()
                .px(8.)
                .gap(8.)
                .center_y()
                .radius(5.)
                .hover(t.hover)
                .role("button")
                .label("Switch workspace")
                .focusable(true)
                .child(label(&s.workspace.icon, 20., t.text))
                .child(
                    text(&s.workspace.name)
                        .font(TextStyle::new(14., 600.))
                        .color(t.text)
                        .truncate()
                        .grow(1.)
                        .min_w(0.),
                )
                .child(label("⌄", 14., t.muted))
                .on_press(|s: &mut State| s.modal = Some(Modal::Workspace)),
        )
        .child(nav("search-button", "⌕", "Search", t).on_press(|s: &mut State| s.open_search()))
        .child(
            nav("home-button", "⌂", "Home", t).on_press_tx(|s: &mut State, tx| {
                tx.instant();
                if let Some(p) = s.pages.first() {
                    let id = p.id.clone();
                    s.open_page(&id, false);
                }
            }),
        )
        .child(el().h(18.))
        .child(
            row()
                .w_full()
                .h(26.)
                .px(10.)
                .center_y()
                .child(label("Private", 12., t.faint))
                .child(el().grow(1.))
                .child(
                    icon_button("sidebar-new-page", "New page", "+", t)
                        .size(22., 22.)
                        .on_press(|s: &mut State| {
                            s.new_name = TextEdit::new("");
                            s.modal = Some(Modal::NewPage);
                        }),
                ),
        )
        .child(
            col()
                .key("sidebar-scroll")
                .grow(1.)
                .min_h(0.)
                .w_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(page_rows),
        )
        .child(
            nav("add-page", "+", "New page", t).on_press(|s: &mut State| {
                s.new_name = TextEdit::new("");
                s.modal = Some(Modal::NewPage);
            }),
        )
        .child(el().h(12.))
        .child(
            nav("settings-button", "⚙", "Settings", t)
                .on_press(|s: &mut State| s.modal = Some(Modal::Settings)),
        )
        .child(nav("trash-button", "♧", "Trash", t).on_press(|s: &mut State| s.open_trash()))
        .child(el().h(14.))
}
pub fn content(s: &State, t: Theme) -> El {
    let top = row()
        .key("page-toolbar")
        .h(42.)
        .w_full()
        .px(16.)
        .center_y()
        .gap(6.)
        .children(s.tab().map(|tab| {
            label(
                format!(
                    "{}  {}",
                    tab.page.icon,
                    if tab.title.text.is_empty() {
                        "Untitled"
                    } else {
                        &tab.title.text
                    }
                ),
                13.,
                t.text,
            )
            .truncate()
        }))
        .child(el().grow(1.))
        .children(s.tab().map(|_| {
            icon_button("page-actions", "Actions", "…", t)
                .on_press(|s: &mut State| s.modal = Some(Modal::PageActions))
        }));
    let body = match s.tab() {
        Some(tab) if tab.database.is_some() => super::database::view(s, t),
        Some(_) => super::editor::view(s, t),
        None => col()
            .grow(1.)
            .w_full()
            .center()
            .gap(16.)
            .child(label("Your workspace", 24., t.text))
            .child(
                button("empty-create", "+ New page", t)
                    .on_press(|s: &mut State| s.modal = Some(Modal::NewPage)),
            ),
    };
    col()
        .key("content")
        .instant_changes()
        .grow(1.)
        .min_w(0.)
        .h_full()
        .fill(t.bg)
        .child(top)
        .child(body)
        .children(s.error.as_ref().map(|err| {
            row()
                .w_full()
                .padding(10.)
                .gap(12.)
                .fill(t.sidebar)
                .child(label(err, 13., t.text).wrap().grow(1.))
                .child(
                    button("dismiss-error", "Dismiss", t).on_press(|s: &mut State| s.error = None),
                )
        }))
}
