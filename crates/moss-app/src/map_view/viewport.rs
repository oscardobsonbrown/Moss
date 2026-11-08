use moss_core::MapBounds;
use rune::Image;

pub(super) const MAX_LAT: f64 = 85.05112878;
const TILE: f64 = 256.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapViewport {
    pub latitude: f64,
    pub longitude: f64,
    pub zoom: f64,
    pub width: f64,
    pub height: f64,
}
impl Default for MapViewport {
    fn default() -> Self {
        Self {
            latitude: 0.,
            longitude: 0.,
            zoom: 2.,
            width: 800.,
            height: 420.,
        }
    }
}
impl MapViewport {
    pub fn from_config(config: &serde_json::Value) -> Self {
        Self::default().with_config(config)
    }
    pub fn with_config(mut self, config: &serde_json::Value) -> Self {
        if let Some(value) = config
            .get("map_center_lat")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
        {
            self.latitude = value.clamp(-MAX_LAT, MAX_LAT);
        }
        if let Some(value) = config
            .get("map_center_lon")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
        {
            self.longitude = value.clamp(-180., 180.);
        }
        if let Some(value) = config
            .get("map_zoom")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite())
        {
            self.zoom = value.clamp(0., 18.);
        }
        self
    }
    fn world(self) -> f64 {
        TILE * 2f64.powf(self.zoom)
    }
    fn xy(self, lat: f64, lon: f64) -> (f64, f64) {
        let world = self.world();
        let rad = lat.clamp(-MAX_LAT, MAX_LAT).to_radians();
        (
            (lon + 180.) / 360. * world,
            (1. - (rad.tan() + 1. / rad.cos()).ln() / std::f64::consts::PI) * world / 2.,
        )
    }
    fn latlon(self, x: f64, y: f64) -> (f64, f64) {
        let world = self.world();
        let lon = x.rem_euclid(world) / world * 360. - 180.;
        let lat = (std::f64::consts::PI * (1. - 2. * y.clamp(0., world) / world))
            .sinh()
            .atan()
            .to_degrees();
        (lat, lon)
    }
    pub fn project(self, lat: f64, lon: f64) -> (f32, f32) {
        let (cx, cy) = self.xy(self.latitude, self.longitude);
        let (x, y) = self.xy(lat, lon);
        let dx = (x - cx + self.world() / 2.).rem_euclid(self.world()) - self.world() / 2.;
        (
            (self.width / 2. + dx) as f32,
            (self.height / 2. + y - cy) as f32,
        )
    }
    pub fn bounds(self) -> MapBounds {
        let (x, y) = self.xy(self.latitude, self.longitude);
        let (north, west) = self.latlon(x - self.width / 2., y - self.height / 2.);
        let (south, east) = self.latlon(x + self.width / 2., y + self.height / 2.);
        if self.width >= self.world() {
            MapBounds {
                south,
                west: -180.,
                north,
                east: 180.,
            }
        } else {
            MapBounds {
                south,
                west,
                north,
                east,
            }
        }
    }
    pub fn pan(self, dx: f32, dy: f32) -> Self {
        if !dx.is_finite() || !dy.is_finite() || (dx == 0. && dy == 0.) {
            return self;
        }
        let (x, y) = self.xy(self.latitude, self.longitude);
        let (latitude, longitude) = self.latlon(x - f64::from(dx), y - f64::from(dy));
        Self {
            latitude,
            longitude,
            ..self
        }
    }
    pub fn zoomed(self, delta: i8) -> Self {
        Self {
            zoom: (self.zoom + f64::from(delta)).clamp(0., 18.),
            ..self
        }
    }
    /// Zoom while keeping the geographic point under the pointer fixed.
    pub fn zoom_by_at(self, delta: f64, x: f32, y: f32) -> Self {
        if !delta.is_finite() || !x.is_finite() || !y.is_finite() {
            return self;
        }
        let next = Self {
            zoom: (self.zoom + delta).clamp(0., 18.),
            ..self
        };
        if next.zoom == self.zoom {
            return self;
        }
        let (cx, cy) = self.xy(self.latitude, self.longitude);
        let px = f64::from(x) - self.width / 2.;
        let py = f64::from(y) - self.height / 2.;
        let factor = next.world() / self.world();
        let (latitude, longitude) = next.latlon((cx + px) * factor - px, (cy + py) * factor - py);
        Self {
            latitude,
            longitude,
            ..next
        }
    }
    /// Fit a geographic extent in this viewport, keeping padding around the pins.
    pub fn fit_bounds(self, bounds: MapBounds, padding: f64) -> Self {
        if !bounds.valid() {
            return self;
        }
        let world = self.world();
        let (west, north) = self.xy(bounds.north, bounds.west);
        let (mut east, south) = self.xy(bounds.south, bounds.east);
        if bounds.west > bounds.east {
            east += world;
        }
        let span_x = ((east - west) / world).max(0.);
        let span_y = ((south - north) / world).max(0.);
        let width = (self.width - 2. * padding.max(0.)).max(1.);
        let height = (self.height - 2. * padding.max(0.)).max(1.);
        let zx = if span_x > 0. {
            (width / (TILE * span_x)).log2()
        } else {
            14.
        };
        let zy = if span_y > 0. {
            (height / (TILE * span_y)).log2()
        } else {
            14.
        };
        let zoom = zx.min(zy).floor().clamp(0., 14.);
        let (latitude, longitude) = self.latlon((west + east) / 2., (north + south) / 2.);
        Self {
            latitude,
            longitude,
            zoom,
            ..self
        }
    }
    pub(super) fn tiles(self) -> Vec<TilePosition> {
        self.tiles_at_zoom(self.tile_zoom())
    }
    pub(super) fn tile_zoom(self) -> u8 {
        self.zoom.ceil().clamp(0., 18.) as u8
    }
    pub(crate) fn tiles_at_zoom(self, zoom: u8) -> Vec<TilePosition> {
        let (cx, cy) = self.xy(self.latitude, self.longitude);
        let left = cx - self.width / 2.;
        let top = cy - self.height / 2.;
        let n = 1i64 << zoom;
        let size = TILE * 2f64.powf(self.zoom - f64::from(zoom));
        let mut out = Vec::new();
        for y in (top / size).floor() as i64..=((top + self.height) / size).floor() as i64 {
            if y < 0 || y >= n {
                continue;
            }
            for x in (left / size).floor() as i64..=((left + self.width) / size).floor() as i64 {
                if out.len() >= 64 {
                    return out;
                }
                out.push(TilePosition {
                    z: zoom,
                    world_x: x,
                    size: size as f32,
                    x: x.rem_euclid(n) as u32,
                    y: y as u32,
                    left: (x as f64 * size - left) as f32,
                    top: (y as f64 * size - top) as f32,
                });
            }
        }
        out
    }
}
#[derive(Clone, Debug)]
pub struct TilePosition {
    pub z: u8,
    pub world_x: i64,
    pub size: f32,
    pub x: u32,
    pub y: u32,
    pub left: f32,
    pub top: f32,
}
#[derive(Clone)]
pub struct MapTile {
    pub position: TilePosition,
    pub image: Image,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dateline_bounds_projection_and_wrapped_tiles_are_consistent() {
        let viewport = MapViewport {
            longitude: 179.,
            zoom: 4.,
            width: 500.,
            height: 300.,
            ..Default::default()
        };
        let bounds = viewport.bounds();
        assert!(bounds.west > bounds.east);
        let (east_x, _) = viewport.project(0., -179.);
        assert!(east_x > 250. && east_x < 300.);
        assert!(viewport.tiles().iter().any(|tile| tile.x == 0));
        let panned = viewport.pan(-100., 0.);
        assert!((-180. ..=180.).contains(&panned.longitude));
        assert_eq!(viewport.zoomed(100).zoom, 18.);
    }

    #[test]
    fn world_bounds_and_polar_clamp_are_valid() {
        let viewport = MapViewport {
            zoom: 1.,
            width: 1200.,
            height: 800.,
            ..Default::default()
        };
        let bounds = viewport.bounds();
        assert!(bounds.valid());
        assert_eq!((bounds.west, bounds.east), (-180., 180.));
        assert!(viewport.pan(0., 1_000_000.).latitude.abs() <= MAX_LAT + 0.0001);
        assert!(viewport.tiles().len() <= 64);
    }
    #[test]
    fn zoom_at_preserves_the_pointer_location() {
        let view = MapViewport {
            latitude: 20.,
            longitude: 140.,
            zoom: 4.,
            width: 800.,
            height: 500.,
        };
        let (cx, cy) = view.xy(view.latitude, view.longitude);
        let (lat, lon) = view.latlon(cx + 120. - view.width / 2., cy + 210. - view.height / 2.);
        let zoomed = view.zoom_by_at(1., 120., 210.);
        let (x, y) = zoomed.project(lat, lon);
        assert!((x - 120.).abs() < 0.01 && (y - 210.).abs() < 0.01);
        let smooth = view.zoom_by_at(0.125, 120., 210.);
        assert_eq!(smooth.zoom, 4.125);
        assert_eq!(smooth.tile_zoom(), 5);
        let (x, y) = smooth.project(lat, lon);
        assert!((x - 120.).abs() < 0.01 && (y - 210.).abs() < 0.01);
        let tile = &smooth.tiles()[0];
        assert!((tile.size - 256. * 2f32.powf(-0.875)).abs() < 0.01);
        assert_eq!(
            MapViewport::from_config(&serde_json::json!({"map_zoom":4.125})).zoom,
            4.125
        );
    }
    #[test]
    fn fit_bounds_keeps_points_inside_padding_and_single_pin_has_max_zoom() {
        let view = MapViewport {
            width: 800.,
            height: 400.,
            ..Default::default()
        };
        let bounds = MapBounds {
            south: -31.95,
            north: 40.,
            west: -70.,
            east: 115.86,
        };
        let fitted = view.fit_bounds(bounds, 40.);
        for (lat, lon) in [(bounds.south, bounds.west), (bounds.north, bounds.east)] {
            let (x, y) = fitted.project(lat, lon);
            assert!((39.9..=760.1).contains(&x));
            assert!((39.9..=360.1).contains(&y));
        }
        let single = view.fit_bounds(
            MapBounds {
                south: 3.,
                north: 3.,
                west: 4.,
                east: 4.,
            },
            40.,
        );
        assert_eq!(single.zoom, 14.);
        let (x, y) = single.project(3., 4.);
        assert!((x - 400.).abs() < 0.01 && (y - 200.).abs() < 0.01);
    }
}
