use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_path_buf();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let base = env::temp_dir().join(format!("textflow-scaffold-{}-{stamp}", std::process::id()));
    fs::create_dir(&base).expect("temporary directory");
    let matrix: &[&[&str]] = &[
        &[],
        &["unicode"],
        &["bidi"],
        &["shaping"],
        &["alloc"],
        &["complex-shaping"],
        &["script-arabic"],
        &["script-thai"],
        &["script-devanagari"],
        &["alloc", "script-arabic", "script-thai", "script-devanagari"],
    ];
    for (index, features) in matrix.iter().enumerate() {
        let dir = base.join(format!("case-{index}"));
        fs::create_dir(&dir).expect("case directory");
        let selected: Vec<_> = features.iter().map(|item| (*item).to_string()).collect();
        for file in textflow_playground::files(&selected).expect("known features") {
            let path = dir.join(file.path);
            fs::create_dir_all(path.parent().expect("file parent")).expect("file parent");
            let content = if file.path == "Cargo.toml" {
                let source = format!("version = \"{}\"", textflow_playground::catalog().version);
                file.content
                    .replace(&source, &format!("path = {:?}", root.to_string_lossy()))
            } else {
                file.content
            };
            fs::write(path, content).expect("generated file");
        }
        let result = Command::new("cargo")
            .args(["check", "--offline", "--manifest-path"])
            .arg(dir.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", base.join("target"))
            .env("RUSTFLAGS", "-Dwarnings")
            .status()
            .expect("cargo check");
        if !result.success() {
            eprintln!("Scaffold case {features:?} failed at {}", dir.display());
            std::process::exit(1);
        }
    }
    fs::remove_dir_all(&base).expect("remove temporary scaffold matrix");
    println!("All {} scaffold combinations compile", matrix.len());
}
