use super::theme::Theme;
use crate::state::State;
use rune::*;

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let cluster = s.database_views.map_cluster.as_ref()?;
    let mut list = col().w_full().max_h(240.).scroll(ScrollSpec {
        stick_bottom: false,
    });
    for record in &cluster.records {
        let id = record.id.clone();
        let db = cluster.database_id.clone();
        let mode = cluster.open_mode.clone();
        let key = format!("map-cluster-record-{id}");
        list = list.child(
            row()
                .key(key.clone())
                .w_full()
                .h(30.)
                .px(8.)
                .gap(8.)
                .center_y()
                .hover(t.hover)
                .radius(4.)
                .role("menuitem")
                .label(format!("Open {}", record.title))
                .focusable(true)
                .child(super::menu::icon_slot(
                    format!("{key}-icon"),
                    rune::icon(&super::phosphor::MAP_PIN, 16.).color(t.muted),
                ))
                .child(
                    t.label(
                        if record.title.is_empty() {
                            "Untitled"
                        } else {
                            &record.title
                        },
                        t.text,
                    )
                    .grow(1.)
                    .min_w(0.)
                    .truncate(),
                )
                .on_press(move |s: &mut State| {
                    s.database_views.map_cluster = None;
                    s.open_database_record(&db, &id, &mode);
                }),
        );
    }
    Some(
        col()
            .key("map-cluster-popup")
            .w(240.)
            .padding(6.)
            .gap(3.)
            .fill(t.menu)
            .radius(10.)
            .border(1., t.line)
            .shadow(6., 20., Color::BLACK.alpha(0.2))
            .role("menu")
            .label("Places at this location")
            .focus_group()
            .focus_return(&cluster.anchor)
            .modal(true, |s: &mut State| s.database_views.map_cluster = None)
            .child(list)
            .anchored_fit(&cluster.anchor, Side::Bottom, Align::Start, 4.),
    )
}
