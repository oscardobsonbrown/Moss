use crate::state::State;
use rune::Tx;

#[derive(Default)]
pub struct DatabaseViews {
    pub calendar: crate::calendar_view::CalendarState,
    pub board: crate::board_view::BoardState,
    pub board_ui: crate::board_interaction::BoardUi,
    pub map_interaction: crate::map_interaction::MapInteraction,
    pub map_cluster: Option<crate::map_clusters::MapCluster>,
    pub chart: crate::chart_view::ChartState,
    pub map: crate::map_view::MapState,
    pub dashboard: crate::dashboard_view::DashboardState,
}

pub fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    let calendar = crate::calendar_view::poll(s, tx);
    let board = crate::board_view::poll(s, time, tx);
    let board_ui = crate::board_interaction::poll(s, time, tx);
    let chart = crate::chart_view::poll(s, time, tx);
    let map = crate::map_view::poll(s, time, tx);
    let dashboard = crate::dashboard_view::poll(s, time, tx);
    let cluster = crate::map_clusters::poll(s);
    let interaction = crate::map_interaction::poll(s, time, tx);
    calendar || board || board_ui || chart || map || dashboard || cluster || interaction
}
