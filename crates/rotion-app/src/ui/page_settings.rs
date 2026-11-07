use super::theme::{Theme, button, label};
use crate::{
    fonts,
    state::{Modal, State},
};
use rotion_core::PageFont;
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
        .modal(true, |s: &mut State| s.modal = None)
        .on_key(|s: &mut State, event, tx| {
            if matches!(event.key, Key::Escape | Key::Left) && s.modal == Some(Modal::Appearance) {
                s.modal = Some(Modal::PageActions);
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
                .child(label("Aa", 18., t.text).w(20.))
                .child(label("Appearance & layout", 14., t.text).grow(1.))
                .child(label("›", 20., t.muted))
                .on_hover(|s: &mut State| s.modal = Some(Modal::Appearance))
                .on_press(|s: &mut State| s.modal = Some(Modal::Appearance))
                .on_key(|s: &mut State, event, tx| {
                    if event.key == Key::Right {
                        s.modal = Some(Modal::Appearance);
                        tx.instant();
                        true
                    } else {
                        false
                    }
                }),
        )
        .child(el().w_full().h(1.).fill(t.line))
        .child(
            button("delete-page", "Move to Trash", t)
                .w_full()
                .h(34.)
                .role("menuitem")
                .on_press_tx(|s: &mut State, tx| {
                    tx.instant();
                    s.modal = None;
                    s.trash_page();
                }),
        )
        .children(
            s.error
                .as_ref()
                .map(|e| label(e, 12., t.text).wrap().padding(8.)),
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
        .child(label("Font style", 12., t.muted).font(TextStyle::new(12., 600.)))
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
                        .child(label(name, 12., t.muted))
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
        .child(label("Preferences", 12., t.muted).font(TextStyle::new(12., 600.)))
        .child(
            toggle(
                "page-small-text",
                "A↓",
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
            toggle("page-full-width", "↔", "Full width", settings.full_width, t).on_press_tx(
                |s: &mut State, tx| {
                    tx.instant();
                    if let Some(tab) = s.tab() {
                        let mut next = tab.page.settings;
                        next.full_width = !next.full_width;
                        s.set_page_settings(next);
                    }
                },
            ),
        )
        .anchored("appearance-menu-item", Side::Left, Align::Start, 6.)
}
fn toggle(key: &str, icon: &str, name: &str, on: bool, t: Theme) -> El {
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
        .child(label(icon, 18., t.text).w(26.))
        .child(label(name, 14., t.text).grow(1.))
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
