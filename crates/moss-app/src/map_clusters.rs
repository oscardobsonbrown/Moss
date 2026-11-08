use crate::map_view::MapViewport;
use moss_core::MapPoint;

#[derive(Clone)]
pub struct ClusterRecord {
    pub id: String,
    pub title: String,
}
#[derive(Clone)]
pub struct MapCluster {
    pub anchor: String,
    pub page_id: String,
    pub view_id: String,
    pub database_id: String,
    pub open_mode: String,
    pub records: Vec<ClusterRecord>,
}

pub fn open(
    s: &mut crate::state::State,
    anchor: String,
    database_id: String,
    open_mode: String,
    records: Vec<ClusterRecord>,
) {
    let Some(tab) = s.tab() else {
        return;
    };
    let Some(view) = tab.database.as_ref().and_then(|db| db.views.get(tab.view)) else {
        return;
    };
    s.database_views.map_cluster = Some(MapCluster {
        anchor,
        page_id: tab.page.id.clone(),
        view_id: view.id.clone(),
        database_id,
        open_mode,
        records,
    });
}
pub fn poll(s: &mut crate::state::State) -> bool {
    let Some(cluster) = &s.database_views.map_cluster else {
        return false;
    };
    let current = s.tab().and_then(|tab| {
        tab.database
            .as_ref()
            .and_then(|db| db.views.get(tab.view))
            .map(|view| (&tab.page.id, &view.id))
    });
    if current != Some((&cluster.page_id, &cluster.view_id)) {
        s.database_views.map_cluster = None;
        true
    } else {
        false
    }
}

pub struct ProjectedCluster {
    pub x: f32,
    pub y: f32,
    pub first_index: usize,
    pub records: Vec<ClusterRecord>,
}
/// Group nearby visible pins; the source projection has a fixed 500-record cap.
pub fn project(points: &[MapPoint], viewport: MapViewport) -> Vec<ProjectedCluster> {
    let mut clusters: Vec<ProjectedCluster> = Vec::new();
    for (index, point) in points.iter().enumerate() {
        let (x, y) = viewport.project(point.place.latitude, point.place.longitude);
        if x < -20.
            || y < -20.
            || x > viewport.width as f32 + 20.
            || y > viewport.height as f32 + 20.
        {
            continue;
        }
        let record = ClusterRecord {
            id: point.record_id.clone(),
            title: point.title.clone(),
        };
        if let Some(cluster) = clusters
            .iter_mut()
            .find(|c| (c.x - x).abs() < 24. && (c.y - y).abs() < 24.)
        {
            cluster.records.push(record);
        } else {
            clusters.push(ProjectedCluster {
                x,
                y,
                first_index: index,
                records: vec![record],
            });
        }
    }
    clusters
}
