use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(program: &str, args: &[&str], directory: &Path) -> Result<(), String> {
    let mut command = Command::new(program);
    command.args(args).current_dir(directory);
    if program == "trunk" {
        command.env_remove("NO_COLOR");
    }
    let status = command
        .status()
        .map_err(|error| format!("{program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

fn prepare(root: &Path) -> Result<(), String> {
    let extractor = root.join("tools/extract-docs/Cargo.toml");
    let output = root.join("playground/web/data");
    run(
        "cargo",
        &[
            "run",
            "--quiet",
            "--manifest-path",
            extractor.to_str().ok_or("Invalid extractor path")?,
            "--",
            "--out",
            output.to_str().ok_or("Invalid output path")?,
        ],
        root,
    )?;
    run(
        "wasm-pack",
        &[
            "build",
            "--target",
            "web",
            "--release",
            "--out-dir",
            "web/pkg",
        ],
        &root.join("playground"),
    )
}

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "build".into());
    match command.as_str() {
        "build" => {
            let mut public_url = None;
            while let Some(argument) = args.next() {
                if argument != "--public-url" {
                    return Err(format!("Unknown argument: {argument}"));
                }
                public_url = Some(args.next().ok_or("Missing public URL")?);
            }
            prepare(&root)?;
            let mut trunk_args = vec!["build", "--release"];
            if let Some(ref url) = public_url {
                trunk_args.extend(["--public-url", url]);
            }
            run("trunk", &trunk_args, &root.join("playground/web"))
        }
        "serve" => {
            let port = args.next().unwrap_or_else(|| "8137".into());
            if args.next().is_some() || port.parse::<u16>().is_err() {
                return Err("Usage: cargo xtask serve [port]".into());
            }
            prepare(&root)?;
            run(
                "trunk",
                &["serve", "--port", &port],
                &root.join("playground/web"),
            )
        }
        "check" => run("bash", &["tools/check-playground.sh"], &root),
        _ => Err("Usage: cargo xtask [build|serve|check]".into()),
    }
}
