use super::theme::Theme;
use crate::{
    sidebar_section_menu::SectionMenuMode,
    sidebar_sections::{SidebarSection, SidebarSectionKind},
    state::State,
};
use rune::*;
use std::rc::Rc;

#[derive(Clone, PartialEq)]
pub struct PageDropZone {
    pub section: String,
    pub page: Option<String>,
    pub parent: Option<String>,
    pub rect: [f32; 4],
}

fn header(s: &State, section: &SidebarSection, t: Theme) -> El {
    let id = section.id.clone();
    let compact = s.sidebar_collapsed() && !s.sidebar_reveal.visible;
    let divider = format!("rail-group-{id}");
    let drag_id = id.clone();
    let anchor = format!("section-actions-{id}");
    let menu_anchor = anchor.clone();
    let menu_id = id.clone();
    let rects: Vec<_> = s
        .sidebar_sections
        .sections
        .iter()
        .filter(|item| item.id != id && !item.hidden)
        .filter_map(|item| {
            let target = item.id.clone();
            rune::memo::window_layout_with(&format!("section-header-{}", item.id), move |rect| {
                (target.clone(), rect[1] + rect[3] / 2.)
            })
        })
        .collect();
    let target = s.sidebar_sections.drag_target.as_deref() == Some(id.as_str());
    let page_target = s
        .sidebar_sections
        .page_drag
        .as_ref()
        .is_some_and(|drag| drag.target.as_deref() == Some(id.as_str()));
    row()
        .key(format!("section-header-{id}"))
        .w_full()
        .h(26.)
        .pl(12.)
        .pr(3.)
        .center_y()
        .radius(4.)
        .fill(if page_target {
            t.hover
        } else {
            Color::TRANSPARENT
        })
        .hover_group()
        .hover_motion(Anim::INSTANT, Anim::INSTANT)
        .child(
            el().key(divider)
                .size(20., 1.)
                .absolute(4., 12.5)
                .fill(t.line)
                .opacity(if compact { 1. } else { 0. })
                .opacity_motion(tok::motion::SNAP_EXIT),
        )
        .child(
            row()
                .grow(1.)
                .min_w(0.)
                .h_full()
                .gap(4.)
                .center_y()
                .key(format!("section-toggle-{id}"))
                .role("button")
                .label(format!("{} section", section.name))
                .expanded(!section.collapsed)
                .focusable(!compact)
                .opacity(if compact { 0. } else { 1. })
                .opacity_motion(tok::motion::SNAP_EXIT)
                .child(
                    t.label(&section.name, t.faint)
                        .font(TextStyle::new(14., 500.).family(Family::Sans))
                        .key(format!("{id}-heading"))
                        .truncate()
                        .shrink()
                        .min_w(0.),
                )
                .child(
                    rune::icon(
                        if section.collapsed {
                            &super::sidebar_actions::RIGHT
                        } else {
                            &super::view_icons::TOOLBAR_CARET
                        },
                        12.,
                    )
                    .key(format!("section-caret-{id}"))
                    .color(t.faint)
                    .group_opacity(if section.collapsed { 1. } else { 0. }, 1.),
                )
                .child(el().grow(1.))
                .on_press_tx(move |s: &mut State, tx| {
                    s.toggle_sidebar_section(&id);
                    tx.instant();
                })
                .on_drag(move |s: &mut State, drag, tx| {
                    tx.instant();
                    if drag.phase == Phase::Down {
                        return;
                    }
                    if drag.dy.abs() >= 4. && !drag.cancelled() {
                        let y = drag.window_point()[1];
                        let before = rects
                            .iter()
                            .find(|(_, center)| y < *center)
                            .map(|(id, _)| id.clone())
                            .unwrap_or_default();
                        s.sidebar_sections.dragging = Some(drag_id.clone());
                        s.sidebar_sections.drag_target = Some(before.clone());
                        if drag.phase == Phase::Up {
                            s.reorder_sidebar_section(&drag_id, &before);
                        }
                    }
                    if drag.phase == Phase::Up || drag.cancelled() {
                        s.sidebar_sections.dragging = None;
                        s.sidebar_sections.drag_target = None;
                    }
                }),
        )
        .child(
            super::sidebar_actions::button(
                s,
                anchor,
                &format!("{} section options", section.name),
                &super::sidebar_actions::MORE,
                16.,
                t.hover,
                t,
            )
            .focusable(!compact)
            .opacity(if compact { 0. } else { 1. })
            .opacity_motion(tok::motion::SNAP_EXIT)
            .on_press(move |s: &mut State| {
                s.open_sidebar_section_menu(SectionMenuMode::Actions(menu_id.clone()), &menu_anchor)
            }),
        )
        .children(target.then(|| el().absolute(0., 0.).w_full().h(2.).fill(t.blue)))
}

#[derive(Clone, Copy)]
pub enum Item {
    Header(usize),
    Private(usize),
    Favorite(usize),
    Shortcut(usize, usize),
    New,
}

fn items(s: &State) -> Vec<Item> {
    let mut rows = Vec::new();
    let visible_count = s
        .sidebar_sections
        .sections
        .iter()
        .filter(|section| !section.hidden)
        .count();
    for (index, section) in s.sidebar_sections.sections.iter().enumerate() {
        if section.hidden {
            continue;
        }
        if visible_count > 1 {
            rows.push(Item::Header(index));
        }
        if section.collapsed && visible_count > 1 {
            continue;
        }
        match section.kind {
            SidebarSectionKind::Private => {
                rows.extend((0..s.sidebar_tree.rows.len()).map(Item::Private))
            }
            SidebarSectionKind::Favorites => {
                rows.extend((0..s.sidebar_favorites.pages.len()).map(Item::Favorite))
            }
            SidebarSectionKind::Recents | SidebarSectionKind::Custom => {
                let count = s
                    .sidebar_sections
                    .pages
                    .get(&section.id)
                    .map_or(0, Vec::len);
                rows.extend((0..count).map(|page| Item::Shortcut(index, page)));
            }
        }
    }
    rows.push(Item::New);
    rows
}

fn item_key(s: &State, item: Item) -> String {
    match item {
        Item::Header(i) => format!("section-header-{}", s.sidebar_sections.sections[i].id),
        Item::Private(i) => match &s.sidebar_tree.rows[i] {
            crate::sidebar_tree::SidebarRow::Page { page, .. } => {
                format!("page-{}", page.id)
            }
            crate::sidebar_tree::SidebarRow::Paging { parent, .. } => {
                format!("paging-{}", parent.as_deref().unwrap_or("root"))
            }
        },
        Item::Favorite(i) => format!("favorite-{}", s.sidebar_favorites.pages[i].id),
        Item::Shortcut(i, page) => {
            let section = &s.sidebar_sections.sections[i];
            format!(
                "{}-{}",
                shortcut_prefix(section),
                s.sidebar_sections.pages[&section.id][page].id
            )
        }
        Item::New => "sidebar-new-section-slot".into(),
    }
}

pub fn shortcut_prefix(section: &SidebarSection) -> String {
    if section.kind == SidebarSectionKind::Recents {
        "recent".into()
    } else {
        format!("section-{}", section.id)
    }
}

pub fn page_drag_preview(s: &State, t: Theme) -> Option<El> {
    let drag = s.sidebar_sections.page_drag.as_ref()?;
    Some(
        row()
            .key("sidebar-page-drag-preview")
            .absolute(drag.point[0] + 8., drag.point[1] - 13.)
            .max_w(220.)
            .h(28.)
            .px(8.)
            .gap(8.)
            .center_y()
            .radius(5.)
            .fill(t.menu)
            .border(1., t.line)
            .shadow(2., 8., Color::BLACK.alpha(0.18))
            .child(el().size(20., 20.).center().child(if drag.icon.is_empty() {
                rune::icon(&super::icons::PAGE_BLANK, 16.).color(t.muted)
            } else {
                super::theme::sized_label(&drag.icon, 16., t.text)
            }))
            .child(
                t.label(
                    if drag.title.is_empty() {
                        "Untitled"
                    } else {
                        &drag.title
                    },
                    t.text,
                )
                .min_w(0.)
                .truncate(),
            )
            .inert()
            .raise(),
    )
}

pub fn view(s: &State, t: Theme) -> El {
    let rows = Rc::new(items(s));
    let mut visible = s
        .sidebar_sections
        .sections
        .iter()
        .filter(|section| !section.hidden);
    let first = visible.next();
    let mut section = if visible.next().is_none() {
        first.map(|section| section.id.clone())
    } else {
        None
    };
    let mut zones = Vec::new();
    for item in rows.iter().copied() {
        let (key, page, parent) = match item {
            Item::Header(index) => {
                section = Some(s.sidebar_sections.sections[index].id.clone());
                (
                    format!("section-header-{}", s.sidebar_sections.sections[index].id),
                    None,
                    None,
                )
            }
            Item::Private(index) => match &s.sidebar_tree.rows[index] {
                crate::sidebar_tree::SidebarRow::Page { page, .. } => (
                    format!("page-highlight-{}", page.id),
                    Some(page.id.clone()),
                    page.parent_id.clone(),
                ),
                _ => continue,
            },
            Item::Favorite(index) => {
                let id = &s.sidebar_favorites.pages[index].id;
                (format!("favorite-highlight-{id}"), Some(id.clone()), None)
            }
            Item::Shortcut(index, page) => {
                let item = &s.sidebar_sections.sections[index];
                let id = &s.sidebar_sections.pages[&item.id][page].id;
                (
                    format!("{}-highlight-{id}", shortcut_prefix(item)),
                    Some(id.clone()),
                    None,
                )
            }
            Item::New => break,
        };
        if let Some(section) = section.clone() {
            if let Some(zone) = rune::memo::window_layout_with(&key, move |rect| PageDropZone {
                section: section.clone(),
                page: page.clone(),
                parent: parent.clone(),
                rect,
            }) {
                zones.push(zone);
            }
        }
    }
    let zones = Rc::new(zones);
    let viewport = rune::memo::window_layout_with("sidebar-scroll", |rect| rect).unwrap_or([0.; 4]);
    let keys = rows.clone();
    col()
        .key("page-list")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows::<State>(
            rows.len(),
            30.,
            move |s, i| std::borrow::Cow::Owned(item_key(s, keys[i])),
            move |s, i| match rows[i] {
                Item::Header(i) => header(s, &s.sidebar_sections.sections[i], t),
                Item::Private(i) => super::sidebar_tree::row_at(s, i, t, zones.clone(), viewport),
                Item::Favorite(i) => super::sidebar_tree::page_row(
                    s,
                    &s.sidebar_favorites.pages[i],
                    false,
                    false,
                    0,
                    "favorite",
                    "favorites",
                    zones.clone(),
                    viewport,
                    t,
                ),
                Item::Shortcut(i, page) => {
                    let section = &s.sidebar_sections.sections[i];
                    super::sidebar_tree::page_row(
                        s,
                        &s.sidebar_sections.pages[&section.id][page],
                        false,
                        false,
                        0,
                        &shortcut_prefix(section),
                        &section.id,
                        zones.clone(),
                        viewport,
                        t,
                    )
                }
                Item::New => new_section(s, t),
            },
        )
}

fn new_section(s: &State, t: Theme) -> El {
    let compact = s.sidebar_collapsed() && !s.sidebar_reveal.visible;
    let end_target = s.sidebar_sections.dragging.is_some()
        && s.sidebar_sections.drag_target.as_deref() == Some("");
    let button = row()
        .key("sidebar-new-section")
        .opacity(if compact { 0. } else { 1. })
        .w_full()
        .h(28.)
        .px(5.)
        .gap(7.)
        .center_y()
        .radius(7.)
        .hover(t.hover)
        .role("button")
        .label("New section")
        .focusable(!compact)
        .child(
            row()
                .size(20., 20.)
                .center()
                .child(rune::icon(&super::view_icons::TOOLBAR_PLUS, 14.).color(t.faint)),
        )
        .child(
            t.label("New section", t.faint)
                .font(TextStyle::new(14., 500.).family(Family::Sans)),
        )
        .child(el().grow(1.))
        .child(
            row()
                .key("sidebar-manage-sections")
                .size(20., 20.)
                .center()
                .radius(5.)
                .hover(t.selected)
                .role("button")
                .label("Show or hide sections")
                .focusable(!compact)
                .child(rune::icon(&super::view_icons::TOOLBAR_SETTINGS, 16.).color(t.faint))
                .on_press_tx(|s: &mut State, tx| {
                    s.open_sidebar_section_menu(
                        SectionMenuMode::Visibility,
                        "sidebar-manage-sections",
                    );
                    tx.instant();
                }),
        )
        .on_press(|s: &mut State| {
            s.open_sidebar_section_menu(SectionMenuMode::Create, "sidebar-new-section")
        })
        .children(end_target.then(|| el().absolute(0., 0.).w_full().h(2.).fill(t.blue)));
    row().w_full().h(30.).px(7.).center_y().child(button)
}
