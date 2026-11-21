#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub fn settings(_: &crate::state::State, _: super::theme::Theme) -> Option<rune::El> {
    None
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub fn toast(_: &crate::state::State, _: super::theme::Theme) -> Option<rune::El> {
    None
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub use macos::{settings, toast};

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod macos {
    use super::super::{
        phosphor,
        theme::{Theme, icon_button},
    };
    use crate::{
        state::State,
        updater::{self, Status, Version},
    };
    use rune::*;

    fn small(t: Theme, title: impl Into<String>, color: Color) -> El {
        t.sized_label(title, 13., color)
    }

    fn update_button(key: &str, title: &str, primary: bool, t: Theme) -> El {
        let (fill, ink) = if primary {
            if t.dark {
                (
                    color::hex("#4799E9").unwrap(),
                    color::hex("#121212").unwrap(),
                )
            } else {
                (color::hex("#186FC6").unwrap(), Color::WHITE)
            }
        } else {
            (t.menu, t.text)
        };
        row()
            .key(key)
            .role("button")
            .label(title)
            .focusable(true)
            .h(28.)
            .px(8.)
            .center()
            .radius(5.)
            .border(1., if primary { fill } else { t.line })
            .fill(fill)
            .hover(if primary {
                fill.mix(ink, 0.12)
            } else {
                t.hover
            })
            .child(
                small(t, title, ink)
                    .font(TextStyle::new(12., if primary { 500. } else { 400. }).family(t.family)),
            )
    }

    fn check(s: &mut State, tx: &mut Tx, download: bool) {
        if let Some(client) = &s.updates.client {
            client.check(download);
            tx.instant();
        }
    }

    pub fn settings(s: &State, t: Theme) -> Option<El> {
        let client = s.updates.client.as_ref()?;
        let status = client.snapshot().status;
        let automatic = client.automatic();
        let action = match status {
            Status::Ready(_) | Status::SaveFailed(_) => {
                update_button("settings-restart-update", "Restart to install", true, t)
                    .on_press_tx(updater::restart)
            }
            Status::Available(_) => {
                update_button("settings-download-update", "Download update", true, t)
                    .on_press_tx(|s, tx| check(s, tx, true))
            }
            _ => update_button(
                "check-for-updates",
                if status == Status::Checking {
                    "Checking…"
                } else {
                    "Check for updates"
                },
                false,
                t,
            )
            .disabled(matches!(
                status,
                Status::Checking | Status::Downloading(_) | Status::Restarting(_)
            ))
            .on_press_tx(|s, tx| check(s, tx, false)),
        };
        let feedback = match status {
            Status::Checking => Some(small(t, "Checking for updates…", t.muted)),
            Status::Current => Some(small(t, "Moss is up to date", t.muted)),
            Status::CheckFailed => {
                Some(small(t, "Couldn’t check for updates. Try again.", t.error))
            }
            Status::Downloading(percent) => {
                let percent = percent.min(100);
                Some(
                    col()
                        .gap(7.)
                        .child(small(t, "Downloading update", t.muted))
                        .child(
                            el().key("update-download-progress")
                                .role("progressbar")
                                .label(format!("Download progress {percent}%"))
                                .size(220., 4.)
                                .fill(t.line)
                                .radius(2.)
                                .child(
                                    el().size(2.2 * f32::from(percent), 4.)
                                        .radius(2.)
                                        .fill(t.blue),
                                ),
                        ),
                )
            }
            _ => None,
        };
        Some(
            col()
                .key("software-update-settings")
                .w_full()
                .gap(8.)
                .child(small(t, "Software update", t.muted))
                .child(
                    row()
                        .h(34.)
                        .gap(10.)
                        .center_y()
                        .child(
                            small(
                                t,
                                format!("Moss {}", updater::version_label(client.version)),
                                t.text,
                            )
                            .grow(1.),
                        )
                        .child(action),
                )
                .child(
                    row()
                        .center_y()
                        .gap(8.)
                        .child(small(t, "Automatic updates", t.text).grow(1.))
                        .child(
                            el().key("automatic-updates")
                                .size(32., 18.)
                                .radius(9.)
                                .role("switch")
                                .label("Automatic updates")
                                .checked(automatic)
                                .focusable(true)
                                .fill(if automatic { t.blue } else { t.faint })
                                .child(
                                    el().size(14., 14.)
                                        .radius(7.)
                                        .fill(Color::WHITE)
                                        .absolute(if automatic { 16. } else { 2. }, 2.),
                                )
                                .on_press_tx(|s: &mut State, tx| {
                                    updater::toggle(s);
                                    tx.instant();
                                }),
                        ),
                )
                .children(feedback.map(|item| item.live_region(LiveRegion::Polite)))
                .child(el().h(1.).w_full().fill(t.line)),
        )
    }

    fn notes(version: Version, t: Theme) -> El {
        super::super::theme::link_button("update-release-notes", "Release notes", t)
            .label("Release notes on GitHub")
            .on_press_tx(move |s: &mut State, tx| {
                if let Err(error) = tx.open_url(&updater::release_url(version)) {
                    s.error = Some(error.to_string());
                }
            })
    }

    pub fn toast(s: &State, t: Theme) -> Option<El> {
        let client = s.updates.client.as_ref()?;
        let view = client.snapshot();
        let (title, glyph, action, release, detail, dismiss) = match view.status {
            Status::Available(next) => (
                "A new version is available".to_owned(),
                &phosphor::ARROW_DOWN,
                Some(
                    update_button("download-update", "Download update", true, t)
                        .on_press_tx(|s, tx| check(s, tx, true)),
                ),
                Some(next),
                None,
                false,
            ),
            Status::Ready(_) => (
                "Update ready".to_owned(),
                &phosphor::CHECK,
                Some(
                    update_button("restart-update", "Restart to install", true, t)
                        .on_press_tx(updater::restart),
                ),
                None,
                None,
                false,
            ),
            Status::SaveFailed(_) => (
                "Save before restarting".to_owned(),
                &phosphor::X,
                Some(
                    update_button("restart-update", "Try again", true, t)
                        .on_press_tx(updater::restart),
                ),
                None,
                Some("Moss couldn’t save an edit. The update is still ready."),
                false,
            ),
            Status::DownloadFailed => (
                "The download was interrupted".to_owned(),
                &phosphor::ARROW_DOWN,
                Some(
                    update_button("retry-update-download", "Retry", false, t)
                        .on_press_tx(|s, tx| check(s, tx, true)),
                ),
                None,
                None,
                true,
            ),
            Status::Unverified => (
                "Moss couldn’t verify this update".to_owned(),
                &phosphor::X,
                Some(
                    update_button("retry-update-check", "Check again", false, t)
                        .on_press_tx(|s, tx| check(s, tx, false)),
                ),
                None,
                None,
                true,
            ),
            Status::InstallLocation => (
                "Moss can’t update from here".to_owned(),
                &phosphor::X,
                None,
                None,
                Some("Move Moss to your Applications folder to update."),
                true,
            ),
            Status::Restarting(_) => return None,
            _ => {
                let version = view.updated?;
                (
                    format!("Updated to Moss {}", updater::version_label(version)),
                    &phosphor::CHECK,
                    None,
                    Some(version),
                    None,
                    true,
                )
            }
        };
        let stacked = matches!(view.status, Status::Available(_) | Status::SaveFailed(_));
        Some(
            col()
                .key("software-update-toast")
                .pin(None, None, Some(16.), Some(16.))
                .w(328.)
                .padding(10.)
                .gap(6.)
                .fill(t.menu)
                .border(1., t.line)
                .radius(8.)
                .shadow(5., 18., Color::BLACK.alpha(0.14))
                .live_region(LiveRegion::Polite)
                .child(
                    row()
                        .gap(7.)
                        .center_y()
                        .child(rune::icon(glyph, 16.).color(t.muted))
                        .child(small(t, title, t.text).wrap().grow(1.).min_w(0.))
                        .children(release.map(|version| notes(version, t)))
                        .children((!stacked).then_some(action.clone()).flatten())
                        .children(dismiss.then(|| {
                            icon_button("dismiss-update-toast", "Dismiss", "×", t)
                                .size(20., 20.)
                                .on_press(|s: &mut State| {
                                    if let Some(client) = &s.updates.client {
                                        client.dismiss();
                                    }
                                })
                        })),
                )
                .children(detail.map(|text| small(t, text, t.muted).wrap()))
                .children(stacked.then_some(action).flatten().map(|button| {
                    row()
                        .w_full()
                        .gap(6.)
                        .child(el().grow(1.))
                        .children(matches!(view.status, Status::SaveFailed(_)).then(|| {
                            update_button("keep-editing-after-update", "Keep editing", false, t)
                                .on_press(|s: &mut State| {
                                    if let Some(client) = &s.updates.client {
                                        client.keep_editing();
                                    }
                                })
                        }))
                        .child(button)
                })),
        )
    }
}
