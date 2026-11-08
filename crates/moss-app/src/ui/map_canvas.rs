use super::theme::Theme;
use crate::{
    map_view::{MapTile, MapViewport},
    state::State,
};
use moss_core::MapPoint;
use rune::*;
use std::collections::BTreeMap;

/// Render independently loaded points; dashboard widgets can supply their own tiles and data.
pub fn canvas(
    points: &[MapPoint],
    tiles: &[MapTile],
    viewport: MapViewport,
    t: Theme,
    database_id: &str,
    open_mode: &str,
    key_prefix: &str,
) -> El {
    let mut canvas = el()
        .key(format!("{key_prefix}-canvas"))
        .w_full()
        .h(viewport.height as f32)
        .clip_rounded(0.)
        .fill(t.sidebar);
    let mut ready = BTreeMap::<_, BTreeMap<_, _>>::new();
    for tile in tiles {
        ready
            .entry(tile.position.z)
            .or_default()
            .insert((tile.position.x, tile.position.y), &tile.image);
    }
    // Input can change the camera after poll. Project tiles and pins from the same camera.
    for (z, images) in ready {
        for position in viewport.tiles_at_zoom(z) {
            if let Some(image) = images.get(&(position.x, position.y)) {
                canvas = canvas.child(
                    el().key(format!(
                        "{key_prefix}-tile-{}-{}-{}-{}",
                        z, position.world_x, position.x, position.y
                    ))
                    .absolute(position.left, position.top)
                    .size(position.size, position.size)
                    .pixels(image.pixels()),
                );
            }
        }
    }
    for cluster in crate::map_clusters::project(points, viewport) {
        let anchor = format!("{key_prefix}-marker-{}", cluster.first_index);
        let db = database_id.to_owned();
        let mode = open_mode.to_owned();
        let records = cluster.records;
        let count = records.len();
        let label = if count == 1 {
            format!("Open {}", records[0].title)
        } else {
            format!("Open {count} places")
        };
        let target_anchor = anchor.clone();
        let mut marker = row()
            .key(anchor)
            .absolute(cluster.x - 10., cluster.y - 20.)
            .size(20., 24.)
            .center()
            .radius(10.)
            .fill(t.blue)
            .role("button")
            .label(label)
            .focusable(true)
            .cursor(Cursor::Pointer);
        marker = if count == 1 {
            marker.child(rune::icon(&super::phosphor::MAP_PIN, 16.).color(Color::WHITE))
        } else {
            marker.child(t.label(count.to_string(), Color::WHITE))
        };
        canvas = canvas.child(marker.on_press(move |s: &mut State| {
            if records.len() == 1 {
                s.open_database_record(&db, &records[0].id, &mode);
            } else {
                crate::map_clusters::open(
                    s,
                    target_anchor.clone(),
                    db.clone(),
                    mode.clone(),
                    records.clone(),
                );
            }
        }));
    }
    canvas.child(
        row()
            .pin(None, None, Some(4.), Some(4.))
            .padding(3.)
            .radius(3.)
            .fill(t.bg)
            .child(t.label(crate::map_view::attribution(), t.text)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_tiles_follow_live_dateline_camera_and_repeat_each_world_copy_once() {
        let path = std::env::temp_dir().join(format!("moss-map-seam-{}", moss_core::new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        let viewport = MapViewport {
            longitude: 179.,
            zoom: 3.,
            width: 600.,
            height: 420.,
            ..Default::default()
        };
        let loader = crate::map_view::MapTileLoader::fake_cached(viewport, path.clone());
        let tile = loader.tiles[0].clone();
        let old = tile.position.clone();
        s.database_views.map.viewport = viewport;
        s.database_views.map.tiles = loader.tiles.clone();
        let mut rt = Runtime::new(
            s,
            |s, _| {
                let m = &s.database_views.map;
                canvas(
                    &[],
                    &m.tiles,
                    m.viewport,
                    Theme::new(false),
                    "",
                    "side",
                    "map",
                )
            },
            (800., 420.),
            Env::default(),
        );
        rt.frame(0.);
        let old_key = format!("map-tile-{}-{}-{}-{}", old.z, old.world_x, old.x, old.y);
        let before = rt.presented(&old_key).unwrap().0;
        rt.update(|s, tx| {
            s.database_views.map.viewport.longitude = -179.;
            tx.instant();
        });
        rt.frame(0.);
        let next = rt
            .state
            .database_views
            .map
            .viewport
            .tiles_at_zoom(old.z)
            .into_iter()
            .find(|p| p.x == old.x && p.y == old.y)
            .unwrap();
        assert_ne!(next.world_x, old.world_x);
        let new_key = format!("map-tile-{}-{}-{}-{}", next.z, next.world_x, next.x, next.y);
        let after = rt.presented(&new_key).unwrap().0;
        assert!((after[0] - before[0] + 2048. * 2. / 360.).abs() < 0.01);
        assert_eq!(after[2], 256.);
        assert!(rt.presented(&old_key).is_none());
        let viewport = MapViewport {
            zoom: 0.,
            width: 800.,
            height: 420.,
            ..Default::default()
        };
        let loader = crate::map_view::MapTileLoader::fake_cached(viewport, path.clone());
        rt.update(move |s, tx| {
            s.database_views.map.viewport = viewport;
            s.database_views.map.tiles = loader.tiles.clone();
            tx.instant();
        });
        rt.frame(0.);
        let mut tiles: Vec<_> = rt
            .elements()
            .into_iter()
            .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("map-tile-")))
            .collect();
        tiles.sort_by(|a, b| a.rect[0].total_cmp(&b.rect[0]));
        assert!(tiles.len() >= 3);
        for pair in tiles.windows(2) {
            assert_ne!(pair[0].key, pair[1].key);
            assert!((pair[1].rect[0] - pair[0].rect[0] - 256.).abs() < 0.01);
        }
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
