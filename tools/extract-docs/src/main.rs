//! Extract API doc metadata from the textflow sources into features.json / docs.json.

mod model;
mod parse;

use anyhow::{bail, Context, Result};
use model::{CrateInfo, DocsFile, Feature, FeatureFile, Item, Module};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

type Reexport = (String, Vec<String>, Vec<String>);

fn main() -> Result<()> {
    let mut out_dir = PathBuf::from("playground/web/data");
    let mut check = false;
    let mut write = true;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--out requires a directory argument")?;
                out_dir = PathBuf::from(value);
            }
            "--check" => check = true,
            "--no-write" => write = false,
            "--help" | "-h" => {
                println!(
                    "usage: extract-docs [--out <dir>] [--check] [--no-write]\n\
                     \n\
                     --out <dir>  output directory (default playground/web/data)\n\
                     --check      print the undocumented public item inventory\n\
                     --no-write   analyse without writing files"
                );
                return Ok(());
            }
            other => bail!("unknown argument: {other}"),
        }
        index += 1;
    }

    let root = find_root()?;
    let feature_file = build_features(&root)?;
    let docs_file = build_docs(&root)?;

    if write {
        std::fs::create_dir_all(&out_dir)?;
        let features_path = out_dir.join("features.json");
        let docs_path = out_dir.join("docs.json");
        std::fs::write(&features_path, serde_json::to_string_pretty(&feature_file)?)?;
        std::fs::write(&docs_path, serde_json::to_string_pretty(&docs_file)?)?;
        eprintln!(
            "wrote {} and {}",
            features_path.display(),
            docs_path.display()
        );
    }

    if check {
        print_inventory(&feature_file, &docs_file);
    }

    Ok(())
}

fn find_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.exists() {
            let content = std::fs::read_to_string(&manifest)?;
            if content.contains("name = \"textflow-rs\"") {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            bail!("textflow-rs repository root not found (run inside the repo)");
        }
    }
}

fn build_features(root: &Path) -> Result<FeatureFile> {
    let content = std::fs::read_to_string(root.join("Cargo.toml"))?;
    let manifest: toml::Value = toml::from_str(&content)?;

    let package = manifest
        .get("package")
        .context("Cargo.toml has no [package]")?;
    let name = package
        .get("name")
        .and_then(|v| v.as_str())
        .context("[package].name missing")?
        .to_string();
    let version = package
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let lib_name = manifest
        .get("lib")
        .and_then(|lib| lib.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or(name.as_str())
        .to_string();

    let descriptions = readme_descriptions(root)?;

    let mut features = Vec::new();
    if let Some(table) = manifest.get("features").and_then(|f| f.as_table()) {
        for (feature_name, value) in table {
            let requires = value
                .as_array()
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| entry.as_str())
                        // Drop "dep:xxx" and "xxx?/yyy"; only real feature edges remain.
                        .filter(|entry| !entry.starts_with("dep:") && !entry.contains('/'))
                        .map(|entry| entry.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            features.push(Feature {
                description: descriptions.get(feature_name).cloned().unwrap_or_default(),
                name: feature_name.clone(),
                requires,
            });
        }
    }

    Ok(FeatureFile {
        crate_info: CrateInfo {
            package: name,
            lib: lib_name,
            version,
        },
        features,
    })
}

fn readme_descriptions(root: &Path) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let Ok(content) = std::fs::read_to_string(root.join("README.md")) else {
        return Ok(map);
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }
        let cells: Vec<&str> = trimmed.split('|').collect();
        if cells.len() < 3 {
            continue;
        }
        let name = cells[1].trim().trim_matches('`');
        let description = cells[2].trim();
        if !name.is_empty() && !description.is_empty() {
            map.insert(name.to_string(), description.to_string());
        }
    }
    Ok(map)
}

fn build_docs(root: &Path) -> Result<DocsFile> {
    let files = parse::collect_sources(root)?;
    let parsed = parse::parse_files(&files);

    let mut modules = Vec::new();
    let lib_path = "src/lib.rs";
    if !parsed.contains_key(lib_path) {
        bail!("src/lib.rs failed to parse");
    }
    walk_module(root, &parsed, lib_path, "", &[], &mut modules)?;

    // Keep the crate-root path for every public re-export.
    let reexported = crate_root_reexports(&parsed, lib_path)?;
    let root_index = modules
        .iter()
        .position(|module| module.name.is_empty())
        .context("crate root module missing")?;
    for (source_module, names, cfg) in reexported {
        let source_path = format!("src/{}.rs", source_module.replace("::", "/"));
        let source =
            if let Some(module) = modules.iter().find(|module| module.source == source_path) {
                module.clone()
            } else if let Ok(content) = std::fs::read_to_string(root.join(&source_path)) {
                parse::parse_source(&source_path, &content, &[], false)?
            } else {
                continue;
            };
        let mut lifted: Vec<Item> = Vec::new();
        for name in &names {
            if let Some(item) = source.items.iter().find(|item| &item.name == name) {
                let mut copy = clone_item(item);
                copy.features = merge_features(&copy.features, &cfg);
                copy.source = Some(source_path.clone());
                lifted.push(copy);
            }
        }
        for item in lifted {
            if !modules[root_index]
                .items
                .iter()
                .any(|existing| existing.name == item.name && existing.features == item.features)
            {
                modules[root_index].items.push(item);
            }
        }
    }

    // Modules without public items carry no documentation value.
    modules.retain(|module| !module.items.is_empty());

    // Crate root first, the rest sorted by name, so output stays stable.
    let crate_root = modules.remove(root_index);
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    let mut ordered = vec![crate_root];
    ordered.extend(modules);

    Ok(DocsFile { modules: ordered })
}

fn clone_item(item: &Item) -> Item {
    Item {
        kind: item.kind.clone(),
        name: item.name.clone(),
        signature: item.signature.clone(),
        doc: item.doc.clone(),
        features: item.features.clone(),
        line: item.line,
        source: item.source.clone(),
        variants: item.variants.clone(),
        fields: item.fields.clone(),
        methods: item.methods.clone(),
    }
}

fn crate_root_reexports(parsed: &parse::ParsedFiles, lib_path: &str) -> Result<Vec<Reexport>> {
    let (lib_file, _) = parsed.get(lib_path).context("src/lib.rs missing")?;
    let mut out = Vec::new();
    for item in &lib_file.items {
        let syn::Item::Use(use_item) = item else {
            continue;
        };
        if !matches!(use_item.vis, syn::Visibility::Public(_)) {
            continue;
        }
        let cfg = parse::cfg_features(&use_item.attrs);
        let mut entries = Vec::new();
        flatten_public_use(&use_item.tree, &mut Vec::new(), &mut entries);
        for (module, names) in entries {
            if let Some(module) = module {
                out.push((module, names, cfg.clone()));
            }
        }
    }
    Ok(out)
}

fn flatten_public_use(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    out: &mut Vec<(Option<String>, Vec<String>)>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten_public_use(&path.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let mut full = prefix.clone();
            full.push(name.ident.to_string());
            if full.first().map(String::as_str) == Some("crate") {
                full.remove(0);
            }
            if full.len() >= 2 {
                let item = full.pop().expect("length checked above");
                out.push((Some(full.join("::")), vec![item]));
            }
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                flatten_public_use(tree, prefix, out);
            }
        }
        syn::UseTree::Rename(rename) => {
            let mut full = prefix.clone();
            full.push(rename.ident.to_string());
            if full.first().map(String::as_str) == Some("crate") {
                full.remove(0);
            }
            if full.len() >= 2 {
                let item = full.pop().expect("length checked above");
                out.push((Some(full.join("::")), vec![item]));
            }
        }
        syn::UseTree::Glob(_) => {
            out.push((Some(prefix.join("::")), vec!["*".into()]));
        }
    }
}

fn walk_module(
    root: &Path,
    parsed: &parse::ParsedFiles,
    file_path: &str,
    module_name: &str,
    inherited: &[String],
    out: &mut Vec<Module>,
) -> Result<()> {
    let (_, children) = parsed
        .get(file_path)
        .with_context(|| format!("{file_path} missing"))?;
    let content = std::fs::read_to_string(root.join(file_path))
        .with_context(|| format!("failed to read {file_path}"))?;
    let module = parse::parse_source(file_path, &content, inherited, module_name.is_empty())?;
    let children = children.clone();
    out.push(module);

    // Children of lib.rs live in src/; other modules keep children in a same-named directory.
    let module_dir = if file_path == "src/lib.rs" {
        PathBuf::from("src")
    } else {
        Path::new(file_path).with_extension("")
    };

    // A module can be declared twice (cfg(not) private vs cfg pub); keep the pub one.
    let mut unique_children: Vec<&parse::ChildModule> = Vec::new();
    for child in &children {
        match unique_children
            .iter()
            .position(|existing| existing.0 == child.0)
        {
            Some(index) => {
                if child.2 && !unique_children[index].2 {
                    unique_children[index] = child;
                }
            }
            None => unique_children.push(child),
        }
    }

    for (child_name, child_cfg, is_pub) in unique_children {
        // Private children of the crate root only serve as re-export sources.
        if module_name.is_empty() && !is_pub {
            continue;
        }
        let child_path = module_dir.join(format!("{child_name}.rs"));
        let child_path = child_path.to_string_lossy().replace('\\', "/");
        if !parsed.contains_key(&child_path) {
            continue; // inline mod or missing file
        }
        let child_module_name = if module_name.is_empty() {
            child_name.clone()
        } else {
            format!("{module_name}::{child_name}")
        };
        let child_inherited = merge_features(inherited, child_cfg);
        walk_module(
            root,
            parsed,
            &child_path,
            &child_module_name,
            &child_inherited,
            out,
        )?;
    }
    Ok(())
}

fn merge_features(base: &[String], extra: &[String]) -> Vec<String> {
    let mut out = base.to_vec();
    for feature in extra {
        if !out.contains(feature) {
            out.push(feature.clone());
        }
    }
    out
}

fn print_inventory(feature_file: &FeatureFile, docs_file: &DocsFile) {
    println!(
        "\n=== {} {} · doc inventory ===",
        feature_file.crate_info.package, feature_file.crate_info.version
    );
    println!(
        "features: {}",
        feature_file
            .features
            .iter()
            .map(|feature| feature.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut total_items = 0;
    let mut documented = 0;
    let mut undocumented: Vec<String> = Vec::new();

    for module in &docs_file.modules {
        for item in &module.items {
            total_items += 1;
            if item.doc.trim().is_empty() {
                undocumented.push(qualified(&module.name, &item.name));
            } else {
                documented += 1;
            }
            for method in &item.methods {
                total_items += 1;
                if method.doc.trim().is_empty() {
                    undocumented.push(qualified(
                        &module.name,
                        &format!("{}::{}", item.name, method.name),
                    ));
                } else {
                    documented += 1;
                }
            }
        }
    }

    println!("modules: {}", docs_file.modules.len());
    println!(
        "public items: {total_items}, documented: {documented}, missing: {}",
        undocumented.len()
    );
    println!(
        "\n--- undocumented public items ({}) ---",
        undocumented.len()
    );
    for path in &undocumented {
        println!("  {path}");
    }
}

fn qualified(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_string()
    } else {
        format!("{module}::{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_reexports_keep_their_definition_source() {
        let root = find_root().unwrap();
        let docs = build_docs(&root).unwrap();
        let root_items = &docs.modules[0].items;
        for (name, source) in [
            ("Line", "src/line.rs"),
            ("LayoutScratch", "src/layout.rs"),
            ("Alignment", "src/layout.rs"),
        ] {
            let item = root_items
                .iter()
                .find(|item| item.name == name)
                .expect("root re-export");
            assert_eq!(item.source.as_deref(), Some(source));
        }
    }

    #[test]
    fn manifest_features_include_new_script() {
        let root = find_root().unwrap();
        let features = build_features(&root).unwrap();
        assert!(features
            .features
            .iter()
            .any(|item| item.name == "script-devanagari"));
    }
}
