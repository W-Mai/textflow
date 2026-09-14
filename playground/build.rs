use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod font_tables;

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .parent()
        .expect("repository root")
        .to_path_buf();
    let manifest_path = root.join("Cargo.toml");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let manifest: toml::Value = fs::read_to_string(&manifest_path)
        .expect("root manifest")
        .parse()
        .expect("valid root manifest");
    let version = manifest["package"]["version"].as_str().expect("version");
    let mut entries = String::new();
    for (name, dependencies) in manifest["features"].as_table().expect("features") {
        let names: Vec<_> = dependencies
            .as_array()
            .expect("feature dependencies")
            .iter()
            .filter_map(toml::Value::as_str)
            .filter(|dependency| !dependency.contains('/') && !dependency.starts_with("dep:"))
            .collect();
        entries.push_str(&format!(
            "({name:?}, &[{}]),\n",
            names
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let rev = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .expect("git rev-parse");
    assert!(rev.status.success(), "git revision unavailable");
    let rev = String::from_utf8(rev.stdout).expect("revision utf8");
    let source = format!(
        "pub const CRATE_VERSION: &str = {version:?};\n\
         pub const SOURCE_REV: &str = {:?};\n\
         pub const FEATURE_GRAPH: &[(&str, &[&str])] = &[\n{entries}];\n",
        rev.trim()
    );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("feature_graph.rs"),
        source,
    )
    .expect("feature graph");
    let font_path = root.join("playground/web/Lato-Regular.ttf");
    println!("cargo:rerun-if-changed={}", font_path.display());
    let font = fs::read(font_path).expect("Playground Latin font");
    let tables = font_tables::generate(&font).expect("valid Playground Latin font");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("latin_font.rs"),
        tables,
    )
    .expect("Latin font tables");
}
