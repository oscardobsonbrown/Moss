use super::theme::{Theme, button, icon_button, label};
use crate::state::{Modal, State};
use rotion_core::BlockKind;
use rune::*;

pub fn overlay(s: &State, t: Theme) -> Option<El> {
    let modal = s.modal?;
    if matches!(modal, Modal::PageActions | Modal::Appearance) {
        return Some(super::page_settings::overlay(s, t));
    }
    let (name, width, body) = match modal {
        Modal::PageActions | Modal::Appearance => unreachable!(),
        Modal::Search => ("Search", 650., search(s, t)),
        Modal::Settings => ("Settings", 520., settings(s, t)),
        Modal::Trash => ("Trash", 600., trash(s, t)),
        Modal::Workspace => ("Workspaces", 360., workspaces(s, t)),
        Modal::Block(i) => ("Block actions", 340., block_menu(s, i, t)),
        Modal::Views => ("Database views", 350., views(s, t)),
        Modal::NewPage => ("New page", 440., new_page(s, t)),
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
                    .children(s.error.as_ref().map(|err| label(err, 13., t.text).wrap())),
            ),
    )
}
fn field(edit: &TextEdit, key: &str, placeholder: &str, t: Theme) -> El {
    text_input(edit, TextStyle::new(16., 400.))
        .key(key)
        .single_line()
        .w_full()
        .h(40.)
        .px(10.)
        .radius(5.)
        .border(1., t.line)
        .color(t.text)
        .placeholder(placeholder, t.faint)
}
fn search(s: &State, t: Theme) -> El {
    let hits = s.hits.iter().enumerate().map(|(i, h)| {
        let id = h.page_id.clone();
        let workspace = h.workspace_id.clone();
        col()
            .key(format!("search-hit-{i}"))
            .w_full()
            .padding(10.)
            .radius(5.)
            .hover(t.hover)
            .role("button")
            .label(&h.title)
            .focusable(true)
            .gap(4.)
            .child(label(&h.title, 15., t.text))
            .child(label(&h.snippet, 12., t.muted).truncate())
            .on_press_tx(move |s: &mut State, tx| {
                tx.instant();
                if workspace == s.workspace.id {
                    s.open_page(&id, true);
                } else {
                    if let Ok(handles) = s.handles.lock() {
                        if let Some(handle) = handles.get(&workspace) {
                            let id = id.clone();
                            handle.update(move |s, tx| {
                                tx.instant();
                                s.open_page(&id, true);
                            });
                            handle.open();
                        }
                    }
                    s.modal = None;
                }
            })
    });
    col()
        .gap(10.)
        .w_full()
        .child(
            field(&s.search, "search-input", "Search pages and databases…", t)
                .label("Search pages and databases")
                .on_edit(|s: &mut State| &mut s.search)
                .on_input_change(|s: &mut State, _| s.run_search()),
        )
        .child(
            button(
                "search-scope",
                if s.all_workspaces {
                    "All workspaces  ⌄"
                } else {
                    "This workspace  ⌄"
                },
                t,
            )
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
                        if s.search.text.is_empty() {
                            "Find a page, a note, or a database record."
                        } else {
                            "No results"
                        },
                        13.,
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
    col().w_full().gap(12.).child(label("Appearance",13.,t.muted)).child(appearance)
        .child(el().h(1.).w_full().fill(t.line))
        .child(label("Workspace",13.,t.muted))
        .child(button("export-workspace","Export workspace archive…",t).on_press_tx(|s:&mut State,tx|{
            let name=format!("{}.rotion",s.workspace.name);
            tx.save_file(FileDialogOptions{title:"Export workspace".into(),file_name:Some(name),..Default::default()}.filter("Rotion archive",&["rotion"]),|s:&mut State,result,_|match result{
                Ok(Some(path))=>match s.store.export_workspace(&s.workspace.id){Ok(data)=>if let Err(e)=std::fs::write(path,data){s.error=Some(e.to_string())},Err(e)=>s.error=Some(e.to_string())},Ok(None)=>{},Err(e)=>s.error=Some(e.to_string())});
        }))
        .child(label("Archive export currently supports pages and databases. Files and media are not yet supported.",12.,t.muted).wrap())
        .child(el().h(1.).w_full().fill(t.line))
        .child(label("Rotion 0.1 · Local development build",12.,t.muted))
}
fn trash(s: &State, t: Theme) -> El {
    col()
        .gap(8.)
        .w_full()
        .child(label("Items stay in Trash for 30 days.", 13., t.muted))
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
                            label(format!("{}  {}", p.icon, p.title), 14., t.text)
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
                        .then(|| label("Trash is empty", 14., t.muted).padding(20.)),
                ),
        )
}
fn workspaces(s: &State, t: Theme) -> El {
    col().gap(4.).children(s.workspaces.iter().map(|w| {
        let id = w.id.clone();
        button(
            format!("workspace-{}", w.id),
            format!(
                "{}  {}{}",
                w.icon,
                w.name,
                if w.id == s.workspace.id { "  ✓" } else { "" }
            ),
            t,
        )
        .w_full()
        .h(38.)
        .on_press_tx(move |s: &mut State, tx| {
            tx.instant();
            s.switch_workspace(&id);
        })
    }))
}
fn block_menu(s: &State, i: usize, t: Theme) -> El {
    let kinds = [
        (BlockKind::Paragraph, "Text"),
        (BlockKind::Heading1, "Heading 1"),
        (BlockKind::Heading2, "Heading 2"),
        (BlockKind::Heading3, "Heading 3"),
        (BlockKind::Todo, "To-do list"),
        (BlockKind::BulletedList, "Bulleted list"),
        (BlockKind::NumberedList, "Numbered list"),
        (BlockKind::Quote, "Quote"),
        (BlockKind::Callout, "Callout"),
        (BlockKind::Code, "Code"),
        (BlockKind::Divider, "Divider"),
    ];
    let current = s.tabs[s.active].blocks[i].block.kind;
    col()
        .gap(2.)
        .children(kinds.into_iter().map(|(kind, name)| {
            button(
                format!("block-kind-{name}"),
                format!("{name}{}", if current == kind { "  ✓" } else { "" }),
                t,
            )
            .w_full()
            .on_press_tx(move |s: &mut State, tx| {
                tx.instant();
                s.tabs[s.active].blocks[i].block.kind = kind;
                s.modal = None;
                s.save_blocks();
            })
        }))
        .child(el().h(1.).w_full().fill(t.line))
        .child(
            button("duplicate-block", "Duplicate", t)
                .w_full()
                .on_press_tx(move |s: &mut State, tx| {
                    tx.instant();
                    let tab = &mut s.tabs[s.active];
                    let mut b = tab.blocks[i].block.clone();
                    b.id = rotion_core::new_id();
                    let mut edit = tab.blocks[i].edit.clone();
                    edit.clear_history();
                    edit.focused = false;
                    tab.blocks
                        .insert(i + 1, crate::state::EditableBlock { block: b, edit });
                    s.modal = None;
                    s.save_blocks();
                }),
        )
        .child(button("delete-block", "Delete", t).w_full().on_press_tx(
            move |s: &mut State, tx| {
                tx.instant();
                let tab = &mut s.tabs[s.active];
                if tab.blocks.len() > 1 {
                    tab.blocks.remove(i);
                } else {
                    tab.blocks[i].edit.replace_text("");
                    tab.blocks[i].block.kind = BlockKind::Paragraph;
                }
                s.modal = None;
                s.save_blocks();
            },
        ))
}
fn views(s: &State, t: Theme) -> El {
    let Some(db) = s.tab().and_then(|t| t.database.as_ref()) else {
        return col();
    };
    col()
        .gap(3.)
        .children(db.views.iter().enumerate().map(|(i, v)| {
            button(format!("select-view-{i}"), &v.name, t)
                .w_full()
                .on_press_tx(move |s: &mut State, tx| {
                    tx.instant();
                    s.tabs[s.active].view = i;
                    s.tabs[s.active].offset = 0;
                    s.modal = None;
                    s.query_records();
                })
        }))
}
fn new_page(s: &State, t: Theme) -> El {
    col()
        .w_full()
        .gap(12.)
        .child(
            field(&s.new_name, "new-page-name", "Untitled", t)
                .label("Page name")
                .on_edit(|s: &mut State| &mut s.new_name)
                .on_submit(|s: &mut State, tx| {
                    tx.instant();
                    s.new_page(false, None);
                }),
        )
        .child(
            button("create-document", "▤  Empty page", t)
                .w_full()
                .h(40.)
                .default_button()
                .on_press_tx(|s: &mut State, tx| {
                    tx.instant();
                    s.new_page(false, None);
                }),
        )
        .child(
            button("create-database", "▦  Database", t)
                .w_full()
                .h(40.)
                .on_press_tx(|s: &mut State, tx| {
                    tx.instant();
                    s.new_page(true, None);
                }),
        )
        .children(s.tab().map(|_| {
            button("create-subpage", "↳  Page inside current page", t)
                .w_full()
                .h(40.)
                .on_press_tx(|s: &mut State, tx| {
                    tx.instant();
                    let parent = s.tab().map(|t| t.page.id.clone());
                    s.new_page(false, parent);
                })
        }))
}
