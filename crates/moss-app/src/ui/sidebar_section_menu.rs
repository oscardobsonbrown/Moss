use super::{menu, phosphor, theme::Theme};
use crate::{
    sidebar_section_menu::SectionMenuMode, sidebar_sections::SidebarSectionKind, state::State,
};
use rune::*;

fn item(key: &str, name: &str, icon: &'static Icon, t: Theme) -> El {
    menu::row_item(key, name, rune::icon(icon, 16.).color(t.muted), t).h(30.)
}
fn rule(t: Theme) -> El {
    el().w_full().h(1.).fill(t.line).my(4.)
}

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let current = s.sidebar_section_menu.as_ref()?;
    let mut body = col().w_full().gap(2.).instant_changes();
    let title;
    match &current.mode {
        SectionMenuMode::Create | SectionMenuMode::Rename(_) => {
            title = if matches!(current.mode, SectionMenuMode::Create) {
                "New section"
            } else {
                "Rename section"
            };
            body = body
                .child(
                    super::inputs::invalid(
                        super::inputs::field(
                            &s.sidebar_section_name,
                            "sidebar-section-name",
                            "Section name",
                            t,
                        ),
                        current.error.is_some(),
                        t,
                    )
                    .label("Section name")
                    .on_edit(|s: &mut State| &mut s.sidebar_section_name)
                    .on_submit(|s: &mut State, tx| {
                        s.save_sidebar_section_name();
                        tx.instant();
                    }),
                )
                .child(
                    item(
                        "sidebar-section-save",
                        if matches!(current.mode, SectionMenuMode::Create) {
                            "Create section"
                        } else {
                            "Save name"
                        },
                        &phosphor::CHECK,
                        t,
                    )
                    .on_press_tx(|s: &mut State, tx| {
                        s.save_sidebar_section_name();
                        tx.instant();
                    }),
                );
        }
        SectionMenuMode::Actions(id) => {
            let index = s
                .sidebar_sections
                .sections
                .iter()
                .position(|item| &item.id == id)?;
            let section = &s.sidebar_sections.sections[index];
            title = "Section options";
            if section.kind == SidebarSectionKind::Custom {
                let target = id.clone();
                let anchor = current.anchor.clone();
                body = body.child(
                    item("sidebar-section-add-page", "Add page", &phosphor::PLUS, t).on_press_tx(
                        move |s: &mut State, tx| {
                            s.open_sidebar_section_menu(
                                SectionMenuMode::AddPage(target.clone()),
                                &anchor,
                            );
                            tx.instant();
                        },
                    ),
                );
                let target = id.clone();
                let anchor = current.anchor.clone();
                body = body
                    .child(
                        item(
                            "sidebar-section-rename",
                            "Rename",
                            &phosphor::NOTE_PENCIL,
                            t,
                        )
                        .on_press_tx(move |s: &mut State, tx| {
                            s.open_sidebar_section_menu(
                                SectionMenuMode::Rename(target.clone()),
                                &anchor,
                            );
                            tx.instant();
                        }),
                    )
                    .child(rule(t));
            }
            for (key, label, icon, offset, visible) in [
                (
                    "sidebar-section-up",
                    "Move up",
                    &phosphor::ARROW_UP,
                    -1,
                    s.sidebar_sections.sections[..index]
                        .iter()
                        .any(|section| !section.hidden),
                ),
                (
                    "sidebar-section-down",
                    "Move down",
                    &phosphor::ARROW_DOWN,
                    1,
                    s.sidebar_sections.sections[index + 1..]
                        .iter()
                        .any(|section| !section.hidden),
                ),
            ] {
                if visible {
                    let target = id.clone();
                    body = body.child(item(key, label, icon, t).on_press_tx(
                        move |s: &mut State, tx| {
                            if s.move_sidebar_section(&target, offset) {
                                s.sidebar_section_menu = None;
                            }
                            tx.instant();
                        },
                    ));
                }
            }
            let target = id.clone();
            body = body.child(
                item(
                    "sidebar-section-collapse",
                    if section.collapsed {
                        "Expand section"
                    } else {
                        "Collapse section"
                    },
                    if section.collapsed {
                        &phosphor::ARROWS_OUT_LINE_VERTICAL
                    } else {
                        &phosphor::ARROWS_IN_LINE_VERTICAL
                    },
                    t,
                )
                .on_press_tx(move |s: &mut State, tx| {
                    if s.toggle_sidebar_section(&target) {
                        s.sidebar_section_menu = None;
                    }
                    tx.instant();
                }),
            );
            let target = id.clone();
            body = body.child(
                item(
                    "sidebar-section-hide",
                    "Hide section",
                    &phosphor::EYE_SLASH,
                    t,
                )
                .on_press_tx(move |s: &mut State, tx| {
                    if s.set_sidebar_section_hidden(&target, true) {
                        s.sidebar_section_menu = None;
                    } else if let Some(menu) = &mut s.sidebar_section_menu {
                        menu.error = s.error.take();
                    }
                    tx.instant();
                }),
            );
            let anchor = current.anchor.clone();
            body = body.child(rule(t)).child(
                item("sidebar-section-new", "New section", &phosphor::PLUS, t).on_press_tx(
                    move |s: &mut State, tx| {
                        s.open_sidebar_section_menu(SectionMenuMode::Create, &anchor);
                        tx.instant();
                    },
                ),
            );
            if section.kind == SidebarSectionKind::Custom {
                let target = id.clone();
                body = body.child(
                    item(
                        "sidebar-section-remove",
                        "Remove section",
                        &phosphor::MINUS,
                        t,
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        if s.remove_sidebar_section(&target) {
                            s.sidebar_section_menu = None;
                        }
                        tx.instant();
                    }),
                );
            }
        }
        SectionMenuMode::Visibility => {
            title = "Show sections";
            let mut sections = col().w_full().min_h(0.).max_h(300.).scroll(ScrollSpec {
                stick_bottom: false,
            });
            for section in &s.sidebar_sections.sections {
                let id = section.id.clone();
                let hidden = section.hidden;
                sections = sections.child(
                    item(
                        &format!("sidebar-section-visibility-{id}"),
                        &section.name,
                        if hidden {
                            &phosphor::EYE_SLASH
                        } else {
                            &phosphor::EYE
                        },
                        t,
                    )
                    .role("checkbox")
                    .checked(!hidden)
                    .child(menu::end(
                        format!("sidebar-section-visibility-check-{id}"),
                        (!hidden).then_some(&phosphor::CHECK),
                        t,
                    ))
                    .on_press_tx(move |s: &mut State, tx| {
                        if !s.set_sidebar_section_hidden(&id, !hidden) {
                            let error = s.error.take();
                            if let Some(menu) = &mut s.sidebar_section_menu {
                                menu.error = error;
                            }
                        }
                        tx.instant();
                    }),
                );
            }
            body = body.child(sections);
        }
        SectionMenuMode::AddPage(section_id) => {
            title = "Add page to section";
            body = body.child(
                super::inputs::field(
                    &s.sidebar_section_search,
                    "sidebar-section-search",
                    "Search pages…",
                    t,
                )
                .label("Search pages")
                .on_edit(|s: &mut State| &mut s.sidebar_section_search)
                .on_input_change(|s: &mut State, tx| {
                    s.run_sidebar_section_search();
                    tx.instant();
                }),
            );
            let section = s
                .sidebar_sections
                .sections
                .iter()
                .find(|item| &item.id == section_id)?;
            let mut pages = col().w_full().min_h(0.).max_h(300.).scroll(ScrollSpec {
                stick_bottom: false,
            });
            for page in &current.pages {
                let id = page.id.clone();
                let section = section.clone();
                let checked = section.page_ids.contains(&id);
                pages = pages.child(
                    menu::row_item(
                        format!("section-pick-{id}"),
                        if page.title.is_empty() {
                            "Untitled"
                        } else {
                            &page.title
                        },
                        if page.icon.is_empty() {
                            rune::icon(&phosphor::FILE, 16.).color(t.muted)
                        } else {
                            t.sized_label(&page.icon, 16., t.text)
                        },
                        t,
                    )
                    .h(30.)
                    .child(menu::end(
                        format!("section-check-{id}"),
                        checked.then_some(&phosphor::CHECK),
                        t,
                    ))
                    .on_press_tx(move |s: &mut State, tx| {
                        s.toggle_sidebar_section_shortcut(&section.id, &id);
                        tx.instant();
                    }),
                );
            }
            body = body.child(pages).children(
                current
                    .pages
                    .is_empty()
                    .then(|| t.label("No pages found", t.faint).padding(10.)),
            );
        }
        SectionMenuMode::ChooseSection(page_id) => {
            title = "Add to section";
            for section in s
                .sidebar_sections
                .sections
                .iter()
                .filter(|section| section.kind == SidebarSectionKind::Custom)
            {
                let id = section.id.clone();
                let page = page_id.clone();
                let checked = section.page_ids.contains(page_id);
                body = body.child(
                    item(
                        &format!("section-choose-{id}"),
                        &section.name,
                        &phosphor::LIST_BULLETS,
                        t,
                    )
                    .child(menu::end(
                        format!("section-choose-check-{id}"),
                        checked.then_some(&phosphor::CHECK),
                        t,
                    ))
                    .on_press_tx(move |s: &mut State, tx| {
                        s.toggle_sidebar_section_shortcut(&id, &page);
                        tx.instant();
                    }),
                );
            }
            if !s
                .sidebar_sections
                .sections
                .iter()
                .any(|section| section.kind == SidebarSectionKind::Custom)
            {
                body = body.child(
                    t.label("Create a section from the sidebar first.", t.faint)
                        .wrap()
                        .padding(10.),
                );
            }
        }
    }
    Some(
        col()
            .key("sidebar-section-menu")
            .w(260.)
            .padding(6.)
            .gap(6.)
            .fill(t.menu)
            .border(1., t.line)
            .radius(8.)
            .shadow(4., 16., Color::BLACK.alpha(0.18))
            .role("menu")
            .label(title)
            .focus_return(&current.anchor)
            .modal(true, |s: &mut State| s.sidebar_section_menu = None)
            .transition(Transition::opacity())
            .anchored_fit(&current.anchor, Side::Right, Align::Start, 4.)
            .child(t.label(title, t.muted).px(10.).py(4.))
            .child(body)
            .children(
                current
                    .error
                    .as_ref()
                    .map(|error| t.label(error, t.error).wrap().px(10.).py(4.)),
            ),
    )
}
