//! Feature colors adapted from the requested Snazzy Maps Apple Map Style.
//! OpenMapTiles data and geometry differ from Google's map; this is a native interpretation.
#[path = "theme/fetch.rs"]
mod fetch;
#[path = "theme/labels.rs"]
mod labels;
#[path = "theme/mvt.rs"]
mod mvt;
pub(super) use fetch::load;

use super::TilePosition;
use std::fmt::Write;

pub(super) const PROVIDER: &str = "moss-apple-v1://openfreemap";
pub(super) const MAX_BYTES: usize = mvt::MAX_BYTES;
pub(super) const SVG_START: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\">";

#[derive(Clone, Copy)]
struct Paint {
    order: u16,
    color: &'static str,
    width: f32,
    stroke: bool,
}
fn paint(layer: &str, class: &str, kind: u8, zoom: u8) -> Option<Paint> {
    let (order, color, width, stroke) = match layer {
        "landcover" => (
            10,
            if class == "ice" { "#f7f1df" } else { "#d0e3b4" },
            0.,
            false,
        ),
        "landuse" => match class {
            "park" | "recreation_ground" | "village_green" | "grass" | "cemetery" => {
                (20, "#bde6ab", 0., false)
            }
            "hospital" => (24, "#fbd3da", 0., false),
            "aerodrome" => (25, "#cfb2db", 0., false),
            _ => (15, "#f7f1df", 0., false),
        },
        "park" => (21, "#bde6ab", 0., false),
        "water" => (30, "#a2daf2", 0., false),
        "waterway" if zoom >= 9 => (
            31,
            "#a2daf2",
            if class == "river" { 1.4 } else { 0.7 },
            true,
        ),
        "aeroway" => (40, "#cfb2db", 2., kind != 3),
        "building" if zoom >= 14 => (50, "#e8dfcb", 0., false),
        "transportation" => {
            let main = matches!(class, "motorway" | "trunk");
            let arterial = matches!(class, "primary" | "secondary" | "tertiary");
            if !main && !arterial && zoom < 12 {
                return None;
            }
            if matches!(class, "rail" | "transit" | "aerialway" | "ferry") {
                return None;
            }
            let width = if main {
                (f32::from(zoom) - 5.).max(1.3) * 0.62
            } else if arterial {
                (f32::from(zoom) - 8.).max(1.) * 0.5
            } else if class == "path" {
                0.45
            } else {
                0.8 + (f32::from(zoom) - 14.).max(0.) * 0.2
            };
            (
                if main { 70 } else { 60 },
                if main {
                    "#ffe15f"
                } else if arterial {
                    "#ffffff"
                } else {
                    "#b9b3a8"
                },
                width,
                true,
            )
        }
        _ => return None,
    };
    Some(Paint {
        order,
        color,
        width,
        stroke,
    })
}
fn property<'a>(feature: &'a mvt::Feature, key: &str) -> &'a str {
    feature
        .properties
        .get(key)
        .map(String::as_str)
        .unwrap_or("")
}
fn screen(x: i64, y: i64, extent: u32, tile: &TilePosition) -> (f32, f32) {
    let source_zoom = tile.z.min(14);
    let subdivisions = 1u32 << (tile.z - source_zoom);
    let width = extent as f32 / subdivisions as f32;
    (
        (x as f32 - (tile.x % subdivisions) as f32 * width) * 256. / width,
        (y as f32 - (tile.y % subdivisions) as f32 * width) * 256. / width,
    )
}
pub(super) fn valid_svg(bytes: &[u8]) -> bool {
    bytes.len() <= MAX_BYTES
        && bytes.starts_with(SVG_START.as_bytes())
        && bytes.ends_with(b"</svg>")
        && ![
            "<!DOCTYPE",
            "<!ENTITY",
            "<script",
            "<image",
            "<foreignObject",
            "href=",
            "url(",
        ]
        .iter()
        .any(|s| bytes.windows(s.len()).any(|w| w == s.as_bytes()))
}

pub(super) fn render(bytes: &[u8], tile: &TilePosition) -> Result<Vec<u8>, String> {
    if tile.z > 18 || tile.x >= (1u32 << tile.z) || tile.y >= (1u32 << tile.z) {
        return Err("Invalid map tile coordinate.".into());
    }
    let layers = mvt::decode(bytes)?;
    let mut shapes = Vec::<(u16, String)>::new();
    let mut names = Vec::<(u32, String, f32, f32, f32)>::new();
    let mut output_size = SVG_START.len();
    for layer in layers {
        for feature in layer.features {
            let class = property(&feature, "class");
            let label =
                layer.name == "place" && matches!(class, "city" | "town" | "country" | "continent");
            let Some(style) = paint(&layer.name, class, feature.kind, tile.z).or_else(|| {
                label.then_some(Paint {
                    order: 100,
                    color: "",
                    width: 0.,
                    stroke: false,
                })
            }) else {
                continue;
            };
            if property(&feature, "brunnel") == "tunnel" {
                continue;
            }
            let paths = mvt::paths(&feature.geometry)?;
            if label {
                let rank = property(&feature, "rank").parse::<u32>().unwrap_or(10);
                if (tile.z < 5 && !matches!(class, "country" | "continent"))
                    || (tile.z >= 7 && matches!(class, "country" | "continent"))
                    || (tile.z < 9 && rank > 4)
                {
                    continue;
                }
                let name = ["name_en", "name:latin", "name"]
                    .iter()
                    .map(|key| property(&feature, key))
                    .find(|v| !v.is_empty())
                    .unwrap_or("");
                if let Some(&(x, y)) = paths.first().and_then(|p| p.first()) {
                    let (x, y) = screen(x, y, layer.extent, tile);
                    if x >= -160. && x <= 416. && y >= -24. && y <= 280. {
                        names.push((
                            rank,
                            name.chars().take(65).collect(),
                            x,
                            y,
                            if class == "city" { 12. } else { 11. },
                        ));
                    }
                }
                continue;
            }
            let mut path = String::new();
            let mut min = [f32::INFINITY; 2];
            let mut max = [f32::NEG_INFINITY; 2];
            for points in paths {
                for (i, (x, y)) in points.into_iter().enumerate() {
                    let (x, y) = screen(x, y, layer.extent, tile);
                    min[0] = min[0].min(x);
                    min[1] = min[1].min(y);
                    max[0] = max[0].max(x);
                    max[1] = max[1].max(y);
                    let _ = write!(path, "{}{x:.2},{y:.2}", if i == 0 { 'M' } else { 'L' });
                }
                if feature.kind == 3 {
                    path.push('Z');
                }
            }
            if path.is_empty() || max[0] < -8. || max[1] < -8. || min[0] > 264. || min[1] > 264. {
                continue;
            }
            let shape = if style.stroke {
                let mut shape = String::new();
                if style.color == "#ffe15f" {
                    let _ = write!(
                        shape,
                        "<path d=\"{path}\" fill=\"none\" stroke=\"#efd151\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                        style.width + 0.8
                    );
                }
                let _ = write!(
                    shape,
                    "<path d=\"{path}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                    style.color, style.width
                );
                shape
            } else {
                format!(
                    "<path d=\"{path}\" fill=\"{}\" fill-rule=\"evenodd\"/>",
                    style.color
                )
            };
            output_size += shape.len();
            if output_size > MAX_BYTES {
                return Err("Rendered map tile is too complex.".into());
            }
            shapes.push((style.order, shape));
        }
    }
    shapes.sort_by_key(|shape| shape.0);
    let mut svg = String::from(SVG_START);
    svg.push_str("<rect width=\"256\" height=\"256\" fill=\"#f7f1df\"/>");
    for (_, shape) in shapes {
        svg.push_str(&shape);
    }
    names.sort_by_key(|name| name.0);
    let mut occupied = Vec::<[f32; 4]>::new();
    for (_, name, x, y, size) in names.into_iter().take(128) {
        if occupied.len() >= 24 {
            break;
        }
        let Some((path, width)) = labels::path(&name, x, y, size) else {
            continue;
        };
        let rect = [
            x - width / 2. - 3.,
            y - size / 2. - 3.,
            width + 6.,
            size + 6.,
        ];
        if occupied.iter().any(|r| {
            r[0] < rect[0] + rect[2]
                && r[0] + r[2] > rect[0]
                && r[1] < rect[1] + rect[3]
                && r[1] + r[3] > rect[1]
        }) {
            continue;
        }
        let _ = write!(
            svg,
            "<g><title>{}</title><path d=\"{path}\" fill=\"#625e56\" stroke=\"#f7f1df\" stroke-width=\"2\" paint-order=\"stroke\" stroke-linejoin=\"round\"/></g>",
            labels::escape(&name)
        );
        occupied.push(rect);
        if svg.len() > MAX_BYTES {
            return Err("Rendered map tile is too complex.".into());
        }
    }
    svg.push_str("</svg>");
    Ok(svg.into_bytes())
}

#[cfg(test)]
#[path = "theme/tests.rs"]
mod tests;
