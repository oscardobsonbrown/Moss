use super::theme::{Theme, button, icon_button, label};
use crate::state::{Modal, State};
use rune::*;

pub fn overlay(s: &State, t: Theme) -> Option<El> {
    let modal = s.modal?;
    if matches!(modal, Modal::PageActions | Modal::Appearance) {
        return Some(super::page_settings::overlay(s, t));
    }
    if modal == Modal::Workspace {
        return Some(super::workspace_picker::popup(s, t));
    }
    if modal == Modal::Views {
        return Some(
            col()
                .key("modal")
                .w(super::menu::WIDTH)
                .min_h(0.)
                .padding(8.)
                .gap(6.)
                .fill(t.menu)
                .radius(8.)
                .border(1., t.line)
                .shadow(4., 16., Color::BLACK.alpha(0.15))
                .role("dialog")
                .label("Database views")
                .modal(true, |s: &mut State| s.modal = None)
                .transition(Transition::opacity())
                .child(views(s, t).instant_changes())
                .children(s.error.as_ref().map(|err| label(err, t.text).wrap()))
                .anchored_fit("db-views", Side::Bottom, Align::End, 4.),
        );
    }
    let (name, width, body) = match modal {
        Modal::PageActions | Modal::Appearance | Modal::Views | Modal::Workspace => unreachable!(),
        Modal::Search => (
            if s.search_new_tab {
                "Open in new tab"
            } else {
                "Search"
            },
            650.,
            search(s, t),
        ),
        Modal::Settings => ("Settings", 520., settings(s, t)),
        Modal::Trash => ("Trash", 600., trash(s, t)),
    };
    Some(
        el().key("modal-scrim")
            .pin(Some(0.), Some(0.), Some(0.), Some(0.))
            .fill(Color::BLACK.alpha(0.18))
            .center()
            .child(
                col()
                    .key("modal")
                    .w(width)
                    .max_h(700.)
                    .fill(t.bg)
                    .radius(9.)
                    .shadow(12., 40., Color::BLACK.alpha(0.18))
                    .border(1., t.line)
                    .padding(14.)
                    .gap(12.)
                    .role("dialog")
                    .label(name)
                    .modal(true, |s: &mut State| s.modal = None)
                    .transition(Transition::opacity())
                    .child(
                        row()
                            .w_full()
                            .center_y()
                            .child(text(name).font(TextStyle::new(14., 600.)).color(t.text))
                            .child(el().grow(1.))
                            .child(
                                icon_button("close-modal", "Close", "×", t)
                                    .on_press(|s: &mut State| s.modal = None),
                            ),
                    )
                    .child(body.instant_changes())
                    .children(s.error.as_ref().map(|err| label(err, t.text).wrap())),
            ),
    )
}
fn search(s: &State, t: Theme) -> El {
    let hits = s.hits.iter().enumerate().map(|(i, h)| {
        row()
            .key(format!("search-hit-{i}"))
            .w_full()
            .h(38.)
            .px(10.)
            .gap(10.)
            .center_y()
            .radius(5.)
            .fill(if i + usize::from(s.search_new_tab) == s.search_selected {
                t.selected
            } else {
                t.bg
            })
            .hover(t.hover)
            .role("button")
            .label(&h.title)
            .focusable(true)
            .child(label(&h.title, t.text).truncate())
            .child(label(&h.snippet, t.muted).truncate().grow(1.))
            .on_press_tx(move |s: &mut State, tx| {
                s.open_search_hit(i);
                tx.instant();
            })
    });
    col()
        .gap(10.)
        .w_full()
        .child(
            super::inputs::field(
                &s.search,
                "search-input",
                if s.search_new_tab {
                    "Open in new tab…"
                } else {
                    "Search pages and databases…"
                },
                t,
            )
            .label("Search pages and databases")
            .on_edit(|s: &mut State| &mut s.search)
            .on_input_change(|s: &mut State, _| s.run_search())
            .on_input_key(|s: &mut State, event, tx| {
                if matches!(event.key, Key::Down | Key::Up) {
                    let count = s.hits.len() + usize::from(s.search_new_tab);
                    if count > 0 {
                        s.search_selected = if event.key == Key::Down {
                            (s.search_selected + 1).min(count - 1)
                        } else {
                            s.search_selected.saturating_sub(1)
                        };
                    }
                    tx.instant();
                    return true;
                }
                false
            })
            .on_submit(|s: &mut State, tx| {
                if s.search_new_tab && s.search_selected == 0 {
                    s.begin_page(None);
                } else {
                    s.open_search_hit(s.search_selected - usize::from(s.search_new_tab));
                }
                tx.instant();
            }),
        )
        .children(s.search_new_tab.then(|| {
            button("search-new-page", "New page", t)
                .w_full()
                .fill(if s.search_selected == 0 {
                    t.selected
                } else {
                    t.bg
                })
                .on_press(|s: &mut State| {
                    s.begin_page(None);
                })
        }))
        .child(
            button(
                "search-scope",
                if s.all_workspaces {
                    "All workspaces"
                } else {
                    "This workspace"
                },
                t,
            )
            .label(if s.all_workspaces {
                "All workspaces  ⌄"
            } else {
                "This workspace  ⌄"
            })
            .gap(6.)
            .child(super::icons::glyph("⌄", 14., t.text))
            .on_press_tx(|s: &mut State, tx| {
                tx.instant();
                s.all_workspaces = !s.all_workspaces;
                s.run_search();
            }),
        )
        .child(
            col()
                .w_full()
                .max_h(440.)
                .min_h(50.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(hits)
                .children((s.hits.is_empty()).then(|| {
                    label(
                        if s.search.text.is_empty() && s.all_workspaces {
                            "Enter a search to find pages in all workspaces."
                        } else if s.search.text.is_empty() {
                            "Find a page, a note, or a database record."
                        } else {
                            "No results"
                        },
                        t.muted,
                    )
                    .padding(12.)
                })),
        )
}
fn settings(s: &State, t: Theme) -> El {
    let appearance = row().gap(6.).children(
        [
            (None, "System"),
            (Some(false), "Light"),
            (Some(true), "Dark"),
        ]
        .into_iter()
        .map(|(dark, name)| {
            button(format!("theme-{name}"), name, t)
                .fill(if s.prefs.dark == dark {
                    t.selected
                } else {
                    t.bg
                })
                .on_press_tx(move |s: &mut State, tx| {
                    tx.instant();
                    s.set_theme(dark);
                })
        }),
    );
    col().w_full().gap(12.).child(label("Appearance",t.muted)).child(appearance)
        .child(label("Start week on", t.muted))
        .child(row().gap(4.).children(["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"].into_iter().enumerate().map(|(day, name)| {
            button(format!("week-start-{}", name.to_lowercase()), &name[..3], t)
                .role("radio")
                .label(format!("Start week on {name}"))
                .checked(s.prefs.week_start() == day as u8)
                .fill(if s.prefs.week_start() == day as u8 { t.selected } else { t.bg })
                .on_press(move |s: &mut State| s.set_week_start(day as u8))
        })))
        .child(el().h(1.).w_full().fill(t.line))
        .child(label("Workspace",t.muted))
        .child(button("export-workspace","Export workspace archive…",t).on_press_tx(|s:&mut State,tx|{
            let name=format!("{}.moss",s.workspace.name);
            tx.save_file(FileDialogOptions{title:"Export workspace".into(),file_name:Some(name),..Default::default()}.filter("Moss archive",&["moss"]),|s:&mut State,result,_|match result{
                Ok(Some(path))=>match s.store.export_workspace(&s.workspace.id){Ok(data)=>if let Err(e)=std::fs::write(path,data){s.error=Some(e.to_string())},Err(e)=>s.error=Some(e.to_string())},Ok(None)=>{},Err(e)=>s.error=Some(e.to_string())});
        }))
        .child(label("Archive export currently supports pages and databases. Files and media are not yet supported.",t.muted).wrap())
        .child(el().h(1.).w_full().fill(t.line))
        .child(label(format!("Moss {}{}", option_env!("MOSS_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")), if cfg!(debug_assertions) { " · Local development build" } else { "" }),t.muted))
}
fn trash(s: &State, t: Theme) -> El {
    col()
        .gap(8.)
        .w_full()
        .child(label("Items stay in Trash for 30 days.", t.muted))
        .child(
            col()
                .w_full()
                .max_h(460.)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(s.trash.iter().enumerate().map(|(i, p)| {
                    let id = p.id.clone();
                    row()
                        .w_full()
                        .h(42.)
                        .gap(10.)
                        .center_y()
                        .child(
                            super::theme::sized_label(
                                format!("{}  {}", p.icon, p.title),
                                14.,
                                t.text,
                            )
                            .grow(1.)
                            .truncate(),
                        )
                        .child(button(format!("restore-{i}"), "Restore", t).on_press_tx(
                            move |s: &mut State, tx| {
                                tx.instant();
                                s.restore_page(&id);
                            },
                        ))
                }))
                .children(
                    s.trash
                        .is_empty()
                        .then(|| label("Trash is empty", t.muted).padding(20.)),
                ),
        )
}
fn views(s: &State, t: Theme) -> El {
    let Some(db) = s.tab().and_then(|t| t.database.as_ref()) else {
        return col();
    };
    let query = s.view_search.text.to_lowercase();
    col()
        .w_full()
        .min_h(0.)
        .shrink()
        .gap(4.)
        .child(
            super::inputs::field(&s.view_search, "view-search", "Search views…", t)
                .label("Search views")
                .on_edit(|s: &mut State| &mut s.view_search)
                .on_input_change(|_: &mut State, tx| tx.instant())
                .on_submit(|s: &mut State, tx| {
                    let query = s.view_search.text.to_lowercase();
                    let index = s.tab().and_then(|t| t.database.as_ref()).and_then(|db| {
                        db.views
                            .iter()
                            .position(|v| v.name.to_lowercase().contains(&query))
                    });
                    if let Some(index) = index {
                        if s.select_database_view(index) {
                            s.modal = None;
                        }
                    }
                    tx.instant();
                }),
        )
        .child(
            col()
                .key("view-options")
                .w_full()
                .max_h(320.)
                .min_h(0.)
                .shrink()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(
                    db.views
                        .iter()
                        .enumerate()
                        .filter(|(_, view)| view.name.to_lowercase().contains(&query))
                        .map(|(i, v)| {
                            super::menu::row_item(
                                format!("select-view-{i}"),
                                &v.name,
                                rune::icon(super::view_icons::for_view(v), 16.)
                                    .color(t.muted)
                                    .key(format!("view-option-icon-{i}")),
                                t,
                            )
                            .child(super::menu::end(
                                format!("select-view-{i}-end"),
                                s.tab()
                                    .is_some_and(|tab| tab.view == i)
                                    .then_some(&super::menu::CHECK),
                                t,
                            ))
                            .on_press_tx(move |s: &mut State, tx| {
                                if s.select_database_view(i) {
                                    s.modal = None;
                                }
                                tx.instant();
                            })
                        }),
                ),
        )
}
