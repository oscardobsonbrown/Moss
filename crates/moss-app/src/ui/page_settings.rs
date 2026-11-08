use super::theme::{Theme, button, label};
use crate::{
    fonts,
    page_appearance::AppearanceSurface,
    state::{Modal, State},
};
use moss_core::PageFont;
use rune::*;

pub fn overlay(s: &State, t: Theme) -> El {
    let appearance = s.modal == Some(Modal::Appearance);
    let menu = col()
        .key("page-actions-menu")
        .w(256.)
        .padding(6.)
        .gap(3.)
        .fill(t.menu)
        .radius(10.)
        .border(1., t.line)
        .shadow(6., 20., Color::BLACK.alpha(0.2))
        .role("menu")
        .label("Page actions")
        .focus_group()
        .focus_return("page-actions")
        .modal(true, |s: &mut State| {
            s.modal = None;
            s.page_appearance = Default::default();
        })
        .on_key(|s: &mut State, event, tx| {
            if matches!(event.key, Key::Escape | Key::Left) && s.modal == Some(Modal::Appearance) {
                s.appearance_close();
                tx.instant();
                return true;
            }
            false
        })
        .child(
            row()
                .key("appearance-menu-item")
                .w_full()
                .h(34.)
                .px(8.)
                .gap(9.)
                .center_y()
                .role("menuitem")
                .label("Appearance & layout")
                .expanded(appearance)
                .focusable(true)
                .radius(5.)
                .hover(t.hover)
                .fill(if appearance { t.hover } else { t.menu })
                .child(
                    rune::icon(&super::phosphor::TEXT_AA, 18.)
                        .color(t.text)
                        .w(20.),
                )
                .child(label("Appearance & layout", t.text).grow(1.))
                .child(rune::icon(&super::phosphor::CARET_RIGHT, 16.).color(t.muted))
                .on_hover_change(|s: &mut State, inside, tx| {
                    s.appearance_hover(AppearanceSurface::Row, inside, tx)
                })
                .on_press(|s: &mut State| s.modal = Some(Modal::Appearance))
                .on_key(|s: &mut State, event, tx| {
                    if matches!(event.key, Key::Right | Key::Enter)
                        || matches!(&event.key, Key::Text(text) if text == " ")
                    {
                        s.appearance_open_keyboard();
                        tx.instant();
                        true
                    } else {
                        false
                    }
                }),
        )
        .children(s.tab().filter(|tab| tab.database.is_some()).map(|_| {
            col()
                .w_full()
                .child(
                    super::menu::row_item(
                        "database-page-icon",
                        "Page icon",
                        rune::icon(&super::phosphor::SMILEY, 16.).color(t.muted),
                        t,
                    )
                    .role("menuitem")
                    .on_hover_change(|s: &mut State, inside, tx| {
                        s.appearance_hover_sibling(inside, tx)
                    })
                    .on_press(|s: &mut State| {
                        if let Some(id) = s.tab().map(|tab| tab.page.id.clone()) {
                            s.modal = None;
                            s.open_page_icon_picker(&id, "page-actions");
                        }
                    }),
                )
                .child(
                    super::menu::row_item(
                        "database-page-cover",
                        "Page cover",
                        rune::icon(&super::phosphor::IMAGES_SQUARE, 16.).color(t.muted),
                        t,
                    )
                    .role("menuitem")
                    .on_hover_change(|s: &mut State, inside, tx| {
                        s.appearance_hover_sibling(inside, tx)
                    })
                    .on_press(|s: &mut State| {
                        if let Some(page) = s.tab().map(|tab| tab.page.clone()) {
                            s.modal = None;
                            s.open_page_cover(&page, "page-actions");
                        }
                    }),
                )
        }))
        .child(el().w_full().h(1.).fill(t.line))
        .child(
            button("create-subpage", "New subpage", t)
                .w_full()
                .h(34.)
                .role("menuitem")
                .on_hover_change(|s: &mut State, inside, tx| s.appearance_hover_sibling(inside, tx))
                .on_press_tx(|s: &mut State, tx| {
                    let parent = s.tab().map(|tab| tab.page.id.clone());
                    s.begin_page(parent);
                    tx.instant();
                }),
        )
        .child(
            row()
                .key("delete-page")
                .w_full()
                .h(34.)
                .px(9.)
                .center_y()
                .radius(4.)
                .hover(t.hover)
                .press(t.selected)
                .role("menuitem")
                .label("Move to Trash")
                .focusable(true)
                .on_hover_change(|s: &mut State, inside, tx| s.appearance_hover_sibling(inside, tx))
                .child(label("Move to Trash", t.error))
                .on_press_tx(|s: &mut State, tx| {
                    tx.instant();
                    s.trash_page();
                }),
        )
        .children(
            s.error
                .as_ref()
                .map(|e| label(e, t.text).wrap().padding(8.)),
        );
    menu.children(appearance.then(|| appearance_panel(s, t)))
        .anchored("page-actions", Side::Bottom, Align::End, 6.)
}
fn appearance_panel(s: &State, t: Theme) -> El {
    let settings = s.tab().map(|t| t.page.settings).unwrap_or_default();
    let previews = [
        (PageFont::Default, "Default", "font-default"),
        (PageFont::Serif, "Serif", "font-serif"),
        (PageFont::Mono, "Mono", "font-mono"),
    ];
    col()
        .key("page-appearance")
        .w(300.)
        .padding(12.)
        .gap(10.)
        .fill(t.menu)
        .radius(10.)
        .border(1., t.line)
        .shadow(6., 20., Color::BLACK.alpha(0.18))
        .role("group")
        .label("Appearance & layout")
        .on_hover_change(|s: &mut State, inside, tx| {
            s.appearance_hover(AppearanceSurface::Panel, inside, tx)
        })
        .child(label("Font style", t.muted).font(TextStyle::new(14., 600.)))
        .child(
            row()
                .w_full()
                .gap(4.)
                .role("radiogroup")
                .label("Font style")
                .focus_group()
                .children(previews.into_iter().map(|(font, name, key)| {
                    col()
                        .key(key)
                        .grow(1.)
                        .h(62.)
                        .center()
                        .gap(5.)
                        .radius(4.)
                        .hover(t.hover)
                        .fill(if settings.font == font {
                            t.hover
                        } else {
                            t.menu
                        })
                        .role("radio")
                        .label(name)
                        .checked(settings.font == font)
                        .focusable(true)
                        .focus_selected(settings.font == font)
                        .child(
                            text("Ag")
                                .font(TextStyle::new(26., 500.).family(fonts::family(font)))
                                .color(if settings.font == font {
                                    t.blue
                                } else {
                                    t.text
                                }),
                        )
                        .child(label(name, t.muted))
                        .on_press_tx(move |s: &mut State, tx| {
                            tx.instant();
                            if let Some(tab) = s.tab() {
                                let mut next = tab.page.settings;
                                next.font = font;
                                s.set_page_settings(next);
                            }
                        })
                })),
        )
        .child(el().w_full().h(1.).fill(t.line))
        .child(label("Preferences", t.muted).font(TextStyle::new(14., 600.)))
        .child(
            toggle(
                "page-small-text",
                &super::phosphor::TEXT_AA,
                "Small text",
                settings.small_text,
                t,
            )
            .on_press_tx(|s: &mut State, tx| {
                tx.instant();
                if let Some(tab) = s.tab() {
                    let mut next = tab.page.settings;
                    next.small_text = !next.small_text;
                    s.set_page_settings(next);
                }
            }),
        )
        .child(
            toggle(
                "page-full-width",
                &super::phosphor::ARROWS_OUT_LINE_HORIZONTAL,
                "Full width",
                settings.full_width,
                t,
            )
            .on_press_tx(|s: &mut State, tx| {
                tx.instant();
                if let Some(tab) = s.tab() {
                    let mut next = tab.page.settings;
                    next.full_width = !next.full_width;
                    s.set_page_settings(next);
                }
            }),
        )
        .anchored("appearance-menu-item", Side::Left, Align::Start, 6.)
}
fn toggle(key: &str, icon: &'static Icon, name: &str, on: bool, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(30.)
        .gap(8.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .role("switch")
        .label(name)
        .checked(on)
        .focusable(true)
        .child(
            row()
                .w(26.)
                .center()
                .child(rune::icon(icon, 18.).color(t.text)),
        )
        .child(label(name, t.text).grow(1.))
        .child(
            el().size(30., 18.)
                .radius(9.)
                .fill(if on { t.blue } else { t.faint })
                .child(
                    el().size(14., 14.)
                        .radius(7.)
                        .fill(Color::WHITE)
                        .absolute(if on { 14. } else { 2. }, 2.),
                ),
        )
}

#[cfg(test)]
#[path = "../page_appearance_tests.rs"]
mod tests;
