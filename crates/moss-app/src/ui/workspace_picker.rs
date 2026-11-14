use super::theme::{Theme, label};
use crate::state::{Modal, State};
use rune::*;

pub fn popup(s: &State, t: Theme) -> El {
    let width = rune::memo::layout_with("workspace-picker", |rect| rect[2]).unwrap_or(254.);
    let header = row()
        .w_full()
        .px(8.)
        .py(6.)
        .gap(8.)
        .center_y()
        .child(
            el().key("workspace-header-icon")
                .size(20., 24.)
                .center()
                .child(super::theme::sized_label(&s.workspace.icon, 16., t.text)),
        )
        .child(
            col()
                .grow(1.)
                .min_w(0.)
                .gap(2.)
                .child(
                    text(&s.workspace.name)
                        .key("workspace-header-name")
                        .font(TextStyle::new(14., 600.))
                        .color(t.text)
                        .truncate(),
                )
                .child(label("Local workspace", t.muted)),
        );
    let settings = item("workspace-settings", "Settings", t)
        .child(
            el().key("workspace-settings-icon")
                .size(20., 24.)
                .center()
                .child(rune::icon(&super::phosphor::GEAR_SIX, 16.).color(t.muted)),
        )
        .child(label("Settings", t.text).key("workspace-settings-label"))
        .on_press(|s: &mut State| s.modal = Some(Modal::Settings));
    let list = col()
        .key("workspace-list")
        .w_full()
        .min_h(0.)
        .max_h(260.)
        .shrink()
        .gap(2.)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .children(s.workspaces.iter().map(|workspace| {
            let id = workspace.id.clone();
            let checked = id == s.workspace.id;
            item(format!("workspace-{id}"), &workspace.name, t)
                .role("menuitemradio")
                .checked(checked)
                .child(
                    el().key(format!("workspace-icon-{id}"))
                        .size(20., 24.)
                        .center()
                        .child(super::theme::sized_label(&workspace.icon, 16., t.text)),
                )
                .child(
                    label(&workspace.name, t.text)
                        .key(format!("workspace-name-{id}"))
                        .min_w(0.)
                        .grow(1.)
                        .truncate(),
                )
                .children(checked.then(|| rune::icon(&super::phosphor::CHECK, 16.).color(t.muted)))
                .on_press_tx(move |s: &mut State, tx| {
                    tx.instant();
                    s.switch_workspace(&id);
                })
        }));
    col()
        .key("workspace-menu")
        .w(width)
        .max_h(520.)
        .min_h(0.)
        .padding(4.)
        .fill(t.menu)
        .radius(10.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.18))
        .role("menu")
        .label("Workspaces")
        .focus_group()
        .focus_return("workspace-picker")
        .modal(true, |s: &mut State| s.modal = None)
        .transition(Transition::opacity())
        .child(
            col()
                .w_full()
                .min_h(0.)
                .shrink()
                .instant_changes()
                .child(header)
                .child(rule(t))
                .child(settings)
                .child(rule(t))
                .child(label("On this Mac", t.muted).px(8.).py(6.))
                .child(list)
                .children(
                    s.error
                        .as_ref()
                        .map(|error| label(error, t.error).wrap().px(10.).py(6.)),
                ),
        )
        .anchored_fit("workspace-picker", Side::Top, Align::Start, 4.)
}

fn rule(t: Theme) -> El {
    el().h(1.).w_full().fill(t.line).my(6.)
}

fn item(key: impl Into<String>, name: &str, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(30.)
        .px(8.)
        .gap(8.)
        .center_y()
        .radius(5.)
        .hover(t.hover)
        .press(t.selected)
        .role("menuitem")
        .label(name)
        .focusable(true)
}
