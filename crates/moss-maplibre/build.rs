#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn main() {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const RELEASE: &str = "core-a33d3f00fb9dbf9117ce18edd799aa0ca5bcce5b";
    const LIB: (&str, &str) = (
        "libmaplibre-native-core-amalgam-macos-arm64-metal.a",
        "faea036b0626ac569cf824eb1ebbfa4d51995b7710065e1afb2e047e94cd30d2",
    );
    const HEADERS: (&str, &str) = (
        "maplibre-native-headers.tar.gz",
        "7ffbb2332ee98cda0c6a74552edcb0428004d75da3595c30682c2ac7dac65791",
    );

    for f in ["build.rs", "src/metal_continuous.mm", "src/metal_cpp_impl.cpp"] {
        println!("cargo:rerun-if-changed={f}");
    }
    println!("cargo:rerun-if-env-changed=MOSS_MAPLIBRE_CACHE_DIR");

    let cache = std::env::var("MOSS_MAPLIBRE_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
            manifest
                .ancestors()
                .nth(2)
                .unwrap_or(&manifest)
                .join("target/maplibre-native-cache")
                .join(RELEASE)
        });
    std::fs::create_dir_all(&cache).expect("create maplibre-native cache dir");

    let sha_ok = |path: &Path, expected: &str| {
        Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .next()
                    .map(|got| got.eq_ignore_ascii_case(expected))
            })
            .unwrap_or(false)
    };

    for (name, sha) in [LIB, HEADERS] {
        let dest = cache.join(name);
        if dest.is_file() && sha_ok(&dest, sha) {
            continue;
        }
        let url = format!("https://github.com/maplibre/maplibre-native/releases/download/{RELEASE}/{name}");
        let tmp = dest.with_extension(format!("tmp.{}", std::process::id()));
        let status = Command::new("curl")
            .args(["--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--proto-redir", "=https", "--retry", "3", "--output"])
            .arg(&tmp)
            .arg(&url)
            .status()
            .expect("invoke curl");
        assert!(status.success(), "Failed to download {url}");
        assert!(sha_ok(&tmp, sha), "SHA-256 mismatch for {name}");
        std::fs::rename(&tmp, &dest).expect("install downloaded file");
    }

    let hdr = cache.join("headers");
    if !hdr.join("include/mln/map/map.hpp").is_file() {
        let tmp = cache.join(format!("headers-tmp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp headers dir");
        assert!(
            Command::new("tar").args(["-xzf"]).arg(cache.join(HEADERS.0)).arg("-C").arg(&tmp).status().expect("tar").success(),
            "Failed to extract maplibre-native headers"
        );
        let _ = std::fs::remove_dir_all(&hdr);
        std::fs::rename(&tmp, &hdr).expect("install headers");
    }

    let mut mm = cc::Build::new();
    mm.cpp(true)
        .file("src/metal_continuous.mm")
        .flags(["-std=c++20", "-fno-rtti", "-fobjc-arc", "-ObjC++", "-Wno-deprecated-declarations"])
        .define("NDEBUG", None);
    for inc in [
        "include",
        "platform/default/include",
        "vendor/maplibre-native-base/include",
        "vendor/maplibre-native-base/deps/variant/include",
        "vendor/maplibre-native-base/deps/geometry.hpp/include",
        "vendor/maplibre-native-base/deps/geojson.hpp/include",
        "vendor/metal-cpp",
        "vendor/expected-lite/include",
    ] {
        mm.include(hdr.join(inc));
    }
    mm.compile("moss_metal_continuous");

    cc::Build::new()
        .cpp(true)
        .file("src/metal_cpp_impl.cpp")
        .flags(["-std=c++20", "-fno-rtti", "-Wno-deprecated-declarations"])
        .define("NDEBUG", None)
        .include(hdr.join("vendor/metal-cpp"))
        .compile("moss_metal_cpp_impl");

    println!("cargo:rustc-link-search=native={}", cache.display());
    println!("cargo:rustc-link-lib=static=maplibre-native-core-amalgam-macos-arm64-metal");
    for lib in ["curl", "z", "sqlite3"] {
        println!("cargo:rustc-link-lib={lib}");
    }
    for fw in ["Metal", "MetalKit", "AppKit", "CoreLocation", "CoreVideo", "IOSurface", "Foundation", "QuartzCore", "CoreGraphics", "CoreText", "ImageIO"] {
        println!("cargo:rustc-link-lib=framework={fw}");
    }
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
