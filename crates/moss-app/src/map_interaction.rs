use crate::state::State;
use rune::{MagnifyEvent, Tx, WheelEvent};
use std::time::Duration;

#[derive(Default)]
pub struct MapInteraction {
    pending: bool,
    changed: bool,
    save_at: Option<f64>,
}
impl MapInteraction {
    pub fn is_moving(&self) -> bool {
        self.pending
    }
}
impl State {
    pub fn map_position_changed(&mut self) {
        self.database_views.map.cancel_fit();
        let interaction = &mut self.database_views.map_interaction;
        interaction.pending = true;
        interaction.changed = true;
    }
    pub fn map_zoom_at(&mut self, delta: i8, x: f32, y: f32) {
        self.map_zoom_by_at(f64::from(delta), x, y);
    }
    fn map_zoom_by_at(&mut self, delta: f64, x: f32, y: f32) {
        let m = &mut self.database_views.map;
        let next = m.viewport.zoom_by_at(delta, x, y);
        if next != m.viewport {
            m.viewport = next;
            m.invalidate();
            self.map_position_changed();
        }
    }
    pub fn map_wheel(&mut self, event: WheelEvent) -> bool {
        if event.mods.cmd {
            if !event.dy.is_finite() || event.dy == 0. {
                return true;
            }
            let delta = if event.precise {
                f64::from(event.dy) / 120.
            } else {
                f64::from(event.dy.signum())
            };
            self.map_zoom_by_at(delta, event.x, event.y);
            return true;
        }
        if !event.precise {
            return false;
        }
        if event.dx.is_finite() && event.dy.is_finite() && (event.dx != 0. || event.dy != 0.) {
            let m = &mut self.database_views.map;
            let next = m.viewport.pan(event.dx, event.dy);
            if next != m.viewport {
                m.viewport = next;
                m.invalidate();
                self.map_position_changed();
            }
        }
        true
    }
    pub fn map_magnify(&mut self, event: MagnifyEvent) {
        let factor = 1. + f64::from(event.delta);
        if factor.is_finite() && factor > 0. {
            self.map_zoom_by_at(factor.log2(), event.x, event.y);
        }
    }
    pub fn finish_map_interaction(&mut self) -> bool {
        if !self.database_views.map_interaction.pending {
            return true;
        }
        let active = self
            .tab()
            .and_then(|t| t.database.as_ref().and_then(|d| d.views.get(t.view)))
            .is_some_and(|v| v.kind == moss_core::ViewKind::Map);
        if !active {
            self.database_views.map_interaction = Default::default();
            return true;
        }
        if self.save_map_viewport() {
            self.database_views.map_interaction = Default::default();
            if self
                .database_views
                .map
                .error
                .as_deref()
                .is_some_and(|e| e.starts_with("Map position is not saved:"))
            {
                self.database_views.map.error = None;
            }
            if self
                .error
                .as_deref()
                .is_some_and(|e| e.starts_with("Map position is not saved:"))
            {
                self.error = None;
            }
            true
        } else {
            self.database_views.map.error = self.error.clone();
            false
        }
    }
}
pub fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    let interaction = &mut s.database_views.map_interaction;
    if !interaction.pending {
        return false;
    }
    if s.database_views.map.drag_start.is_some() {
        tx.wake_after(Duration::from_millis(50));
        return false;
    }
    if interaction.changed {
        interaction.changed = false;
        interaction.save_at = Some(time + 0.18);
    }
    let deadline = interaction.save_at.unwrap_or(time + 0.18);
    if time >= deadline {
        if !s.finish_map_interaction() {
            s.database_views.map_interaction.save_at = Some(time + 1.);
            tx.wake_after(Duration::from_secs(1));
        }
        return true;
    }
    tx.wake_after(Duration::from_secs_f64((deadline - time).max(0.001)));
    false
}
