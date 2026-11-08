use super::*;

fn varint(mut number: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    while number > 127 {
        bytes.push(number as u8 | 128);
        number >>= 7;
    }
    bytes.push(number as u8);
    bytes
}
fn number(field: u64, value: u64) -> Vec<u8> {
    let mut out = varint(field << 3);
    out.extend(varint(value));
    out
}
fn data(field: u64, bytes: &[u8]) -> Vec<u8> {
    let mut out = varint(field << 3 | 2);
    out.extend(varint(bytes.len() as u64));
    out.extend(bytes);
    out
}
fn text(field: u64, value: &str) -> Vec<u8> {
    data(field, value.as_bytes())
}
fn packed(field: u64, words: &[u32]) -> Vec<u8> {
    data(
        field,
        &words
            .iter()
            .flat_map(|word| varint(u64::from(*word)))
            .collect::<Vec<_>>(),
    )
}
fn layer(name: &str, class: &str, kind: u64, geometry: &[u32], label: Option<&str>) -> Vec<u8> {
    let mut out = text(1, name);
    out.extend(text(3, "class"));
    out.extend(data(4, &text(1, class)));
    let mut tags = vec![0, 0];
    if let Some(label) = label {
        out.extend(text(3, "name_en"));
        out.extend(data(4, &text(1, label)));
        tags.extend([1, 1]);
    }
    let mut feature = packed(2, &tags);
    feature.extend(number(3, kind));
    feature.extend(packed(4, geometry));
    out.extend(data(2, &feature));
    out.extend(number(5, 4096));
    out.extend(number(15, 2));
    data(3, &out)
}
fn fixture() -> Vec<u8> {
    let mut bytes = layer(
        "landcover",
        "wood",
        3,
        &[9, 0, 0, 26, 8192, 0, 0, 8192, 8191, 0, 15],
        None,
    );
    bytes.extend(layer(
        "water",
        "ocean",
        3,
        &[9, 0, 0, 26, 4096, 0, 0, 8192, 4095, 0, 15],
        None,
    ));
    bytes.extend(layer(
        "transportation",
        "motorway",
        2,
        &[9, 0, 4096, 10, 8192, 0],
        None,
    ));
    bytes.extend(layer(
        "place",
        "city",
        1,
        &[9, 4096, 2048],
        Some("Test & Town"),
    ));
    bytes
}
fn tile(z: u8, x: u32, y: u32) -> TilePosition {
    TilePosition {
        z,
        x,
        y,
        world_x: i64::from(x),
        size: 256.,
        left: 0.,
        top: 0.,
    }
}

#[test]
fn feature_theme_uses_requested_palette_and_native_glyph_labels() {
    let bytes = render(&fixture(), &tile(12, 1, 1)).unwrap();
    assert!(valid_svg(&bytes));
    let svg = String::from_utf8(bytes.clone()).unwrap();
    for color in ["#f7f1df", "#d0e3b4", "#a2daf2", "#ffe15f", "#efd151"] {
        assert!(svg.contains(color), "missing {color}");
    }
    assert!(svg.contains("Test &amp; Town"));
    assert!(!svg.contains("<text"));
    let cache = rune::ImageCache::new(std::sync::Arc::new(|| {}));
    let image = cache.load(bytes.clone());
    assert_eq!(image.wait().unwrap(), [512, 512]);
    assert_eq!(
        cache.load(bytes).id(),
        image.id(),
        "same themed tile keeps image identity"
    );
    assert!(!valid_svg(
        format!("{SVG_START}<image href=\"file:///private\"/></svg>").as_bytes()
    ));
}

#[test]
fn overzoom_crops_geometry_and_keeps_screen_sized_label_paths() {
    let child = tile(18, (100 << 4) + 3, (100 << 4) + 5);
    let (x, y) = screen(3 * 256 + 128, 5 * 256 + 128, 4096, &child);
    assert_eq!((x, y), (128., 128.));
    let (first, width) = labels::path("Perth", 128., 128., 12.).unwrap();
    let (next, zoomed_width) = labels::path("Perth", x, y, 12.).unwrap();
    assert_eq!(first, next);
    assert_eq!(width, zoomed_width);
    assert!(render(&fixture(), &child).is_ok());
}

#[test]
fn malformed_vector_input_is_bounded_and_rejected() {
    for bytes in [
        vec![26, 255, 255, 255, 255, 127],
        vec![255; 12],
        vec![0; MAX_BYTES + 1],
    ] {
        assert!(mvt::decode(&bytes).is_err());
    }
    for words in [vec![9, 0], vec![15], vec![0], vec![11]] {
        assert!(mvt::paths(&words).is_err());
    }
    let mut bytes = fixture();
    bytes.pop();
    assert!(mvt::decode(&bytes).is_err());
    let mut repeated = text(1, "place");
    repeated.extend(text(3, "name_en"));
    repeated.extend(data(4, &text(1, &"a".repeat(256))));
    let mut feature = packed(2, &[0, 0]);
    feature.extend(number(3, 1));
    feature.extend(packed(4, &[9, 0, 0]));
    for _ in 0..9000 {
        repeated.extend(data(2, &feature));
    }
    repeated.extend(number(5, 4096));
    repeated.extend(number(15, 2));
    let bytes = data(3, &repeated);
    assert!(bytes.len() < MAX_BYTES);
    assert!(
        mvt::decode(&bytes).is_err(),
        "reused tags cannot expand property memory without a bound"
    );
}

#[test]
#[ignore = "requires downloaded public OpenFreeMap PBF fixtures and an output directory"]
fn public_map_theme_fixture_renders_real_features() {
    let input = std::path::PathBuf::from(
        std::env::var_os("MOSS_MAP_THEME_FIXTURES").expect("fixture directory"),
    );
    let output = std::path::PathBuf::from(
        std::env::var_os("MOSS_MAP_THEME_OUTPUT").expect("output directory"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let cache = rune::ImageCache::new(std::sync::Arc::new(|| {}));
    let mut count = 0;
    for entry in std::fs::read_dir(input)
        .unwrap()
        .filter_map(Result::ok)
        .take(32)
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("pbf") {
            continue;
        }
        let parts = path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .split('-')
            .map(str::parse::<u32>)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(parts.len(), 3);
        let tile = tile(parts[0] as u8, parts[1], parts[2]);
        let source = std::fs::read(path).unwrap();
        let bytes = render(&source, &tile).unwrap();
        let svg = std::str::from_utf8(&bytes).unwrap();
        assert!(
            svg.matches("<path").count() > 10,
            "real map must contain geographic features"
        );
        let image = cache.load(bytes.clone());
        assert_eq!(image.wait().unwrap(), [512, 512]);
        std::fs::write(
            output.join(format!("apple-v1-{}-{}-{}.svg", tile.z, tile.x, tile.y)),
            &bytes,
        )
        .unwrap();
        println!(
            "theme {}-{}-{}: {} PBF bytes, {} SVG bytes, {} paths",
            tile.z,
            tile.x,
            tile.y,
            source.len(),
            bytes.len(),
            svg.matches("<path").count()
        );
        count += 1;
    }
    assert!(count > 0);
}
