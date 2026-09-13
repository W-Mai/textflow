//! Parse lib sources with syn into a module/item tree.

use crate::model::{Field, Item, Method, Module, Variant};
use proc_macro2::TokenStream;
use quote::ToTokens;
use std::collections::HashMap;
use syn::{Attribute, Item as SynItem, Visibility};

pub type ChildModule = (String, Vec<String>, bool);
pub type ParsedFiles = HashMap<String, (syn::File, Vec<ChildModule>)>;

pub fn module_doc(attrs: &[Attribute]) -> String {
    doc_lines(attrs, "!")
}

pub fn item_doc(attrs: &[Attribute]) -> String {
    doc_lines(attrs, "")
}

fn doc_lines(attrs: &[Attribute], bang: &str) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(nv) = &attr.meta else {
            continue;
        };
        let syn::Expr::Lit(expr) = &nv.value else {
            continue;
        };
        let syn::Lit::Str(text) = &expr.lit else {
            continue;
        };
        let value = text.value();
        // Module docs carry a leading "!"; item docs are bare text.
        let line = if bang.is_empty() {
            value
        } else {
            value
                .strip_prefix(bang)
                .map(str::to_string)
                .unwrap_or(value)
        };
        lines.push(line.trim_start().to_string());
    }
    lines.join("\n").trim().to_string()
}

pub fn extract_doctests(doc: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<Vec<String>> = None;
    for line in doc.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            match current.take() {
                None => current = Some(vec![line.to_string()]),
                Some(mut block) => {
                    block.push(line.to_string());
                    out.push(block.join("\n"));
                }
            }
        } else if let Some(block) = current.as_mut() {
            block.push(line.to_string());
        }
    }
    out
}

pub fn cfg_features(attrs: &[Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("cfg") {
            continue;
        }
        if attr.parse_nested_meta(collect_feature(&mut out)).is_err() {
            continue;
        }
    }
    out
}

fn collect_feature<'a>(
    out: &'a mut Vec<String>,
) -> impl FnMut(syn::meta::ParseNestedMeta) -> syn::Result<()> + 'a {
    move |meta| {
        if meta.path.is_ident("feature") {
            let value: syn::LitStr = meta.value()?.parse()?;
            if !out.contains(&value.value()) {
                out.push(value.value());
            }
        } else if meta.path.is_ident("all") || meta.path.is_ident("any") {
            let mut inner = Vec::new();
            meta.parse_nested_meta(collect_feature(&mut inner))?;
            for feature in inner {
                if !out.contains(&feature) {
                    out.push(feature);
                }
            }
        }
        Ok(())
    }
}

/// Non-feature gates such as `#[cfg(test)]` drop the whole subtree.
pub fn is_gated_out(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        let Ok(list) = attr.meta.require_list() else {
            return false;
        };
        let tokens = list.tokens.to_string();
        tokens.contains("test")
    })
}

fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn line_of(span: proc_macro2::Span) -> usize {
    span.start().line
}

fn doc_and_line(attrs: &[Attribute], span: proc_macro2::Span) -> (String, usize) {
    (item_doc(attrs), line_of(span))
}

fn path_tail(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    path.path.segments.last().map(|s| s.ident.to_string())
}

fn render(stream: TokenStream) -> String {
    stream.to_string().replace(" ,", ",").replace(" :", ":")
}

fn vis_prefix(vis: &Visibility) -> String {
    match vis {
        Visibility::Public(_) => "pub ".to_string(),
        Visibility::Restricted(r) => format!("{} ", r.to_token_stream()),
        Visibility::Inherited => String::new(),
    }
}

fn generics_of(generics: &syn::Generics) -> String {
    let text = render(generics.to_token_stream());
    if text.trim().is_empty() {
        String::new()
    } else {
        format!(" {}", text.trim())
    }
}

fn struct_signature(item: &syn::ItemStruct) -> String {
    let generics = generics_of(&item.generics);
    match &item.fields {
        syn::Fields::Named(f) => format!(
            "{}struct {}{} {{ /* {} fields */ }}",
            vis_prefix(&item.vis),
            item.ident,
            generics,
            f.named.len()
        ),
        syn::Fields::Unnamed(f) => format!(
            "{}struct {}{}(/* {} fields */);",
            vis_prefix(&item.vis),
            item.ident,
            generics,
            f.unnamed.len()
        ),
        syn::Fields::Unit => format!(
            "{}struct {}{};",
            vis_prefix(&item.vis),
            item.ident,
            generics
        ),
    }
}

fn enum_signature(item: &syn::ItemEnum) -> String {
    format!(
        "{}enum {}{} {{ /* {} variants */ }}",
        vis_prefix(&item.vis),
        item.ident,
        generics_of(&item.generics),
        item.variants.len()
    )
}

fn trait_signature(item: &syn::ItemTrait) -> String {
    let unsafety = if item.unsafety.is_some() {
        "unsafe "
    } else {
        ""
    };
    format!(
        "{}{}trait {}{}",
        vis_prefix(&item.vis),
        unsafety,
        item.ident,
        generics_of(&item.generics)
    )
}

fn fn_signature(sig: &syn::Signature, vis: &Visibility) -> String {
    let inputs: Vec<String> = sig
        .inputs
        .iter()
        .map(|arg| render(arg.to_token_stream()))
        .collect();
    let output = match &sig.output {
        syn::ReturnType::Default => String::new(),
        syn::ReturnType::Type(_, ty) => format!(" -> {}", render(ty.to_token_stream())),
    };
    let asyncness = if sig.asyncness.is_some() {
        "async "
    } else {
        ""
    };
    let unsafety = if sig.unsafety.is_some() {
        "unsafe "
    } else {
        ""
    };
    format!(
        "{}{}{}fn {}({}){}",
        vis_prefix(vis),
        asyncness,
        unsafety,
        sig.ident,
        inputs.join(", "),
        output
    )
}

fn merge(base: &[String], extra: &[String]) -> Vec<String> {
    let mut out = base.to_vec();
    for feature in extra {
        if !out.contains(feature) {
            out.push(feature.clone());
        }
    }
    out
}

/// Methods of an inherent impl block. Trait impls are skipped.
fn collect_methods(
    item_impl: &syn::ItemImpl,
    inherited: &[String],
) -> Option<(String, Vec<Method>)> {
    if item_impl.trait_.is_some() {
        return None;
    }
    let type_name = path_tail(&item_impl.self_ty)?;
    let impl_features = merge(inherited, &cfg_features(&item_impl.attrs));
    if is_gated_out(&item_impl.attrs) {
        return None;
    }
    let mut methods = Vec::new();
    for inner in &item_impl.items {
        let syn::ImplItem::Fn(method) = inner else {
            continue;
        };
        if !is_pub(&method.vis) {
            continue;
        }
        let doc = item_doc(&method.attrs);
        let features = merge(&impl_features, &cfg_features(&method.attrs));
        methods.push(Method {
            name: method.sig.ident.to_string(),
            signature: fn_signature(&method.sig, &method.vis),
            doctests: extract_doctests(&doc),
            line: line_of(method.sig.ident.span()),
            doc,
            features,
        });
    }
    Some((type_name, methods))
}

pub fn parse_source(
    source_path: &str,
    content: &str,
    inherited: &[String],
    is_root: bool,
) -> anyhow::Result<Module> {
    let file: syn::File = syn::parse_str(content)?;
    let module_name = module_path_for(source_path);
    let mut items: Vec<Item> = Vec::new();
    // Impl blocks can precede their type, so collect first and merge after.
    let mut impl_blocks: Vec<(String, Vec<Method>)> = Vec::new();

    for item in &file.items {
        if is_gated_out(item_attrs(item)) {
            continue;
        }
        let features = merge(inherited, &cfg_features(item_attrs(item)));

        match item {
            SynItem::Struct(s) if is_pub(&s.vis) => {
                let (doc, line) = doc_and_line(&s.attrs, s.ident.span());
                let fields = public_fields(&s.fields);
                items.push(Item {
                    kind: "struct".into(),
                    source: None,
                    name: s.ident.to_string(),
                    signature: struct_signature(s),
                    features,
                    doc,
                    line,
                    variants: Vec::new(),
                    fields,
                    methods: Vec::new(),
                });
            }
            SynItem::Enum(e) if is_pub(&e.vis) => {
                let (doc, line) = doc_and_line(&e.attrs, e.ident.span());
                let variants = e
                    .variants
                    .iter()
                    .map(|v| Variant {
                        name: v.ident.to_string(),
                        doc: item_doc(&v.attrs),
                        line: line_of(v.ident.span()),
                    })
                    .collect();
                items.push(Item {
                    kind: "enum".into(),
                    source: None,
                    name: e.ident.to_string(),
                    signature: enum_signature(e),
                    features,
                    doc,
                    line,
                    variants,
                    fields: Vec::new(),
                    methods: Vec::new(),
                });
            }
            SynItem::Trait(t) if is_pub(&t.vis) => {
                let (doc, line) = doc_and_line(&t.attrs, t.ident.span());
                let methods = t
                    .items
                    .iter()
                    .filter_map(|inner| match inner {
                        syn::TraitItem::Fn(method) => Some(Method {
                            name: method.sig.ident.to_string(),
                            signature: fn_signature(&method.sig, &Visibility::Inherited),
                            doc: item_doc(&method.attrs),
                            doctests: extract_doctests(&item_doc(&method.attrs)),
                            line: line_of(method.sig.ident.span()),
                            features: Vec::new(),
                        }),
                        _ => None,
                    })
                    .collect();
                items.push(Item {
                    kind: "trait".into(),
                    source: None,
                    name: t.ident.to_string(),
                    signature: trait_signature(t),
                    features,
                    doc,
                    line,
                    variants: Vec::new(),
                    fields: Vec::new(),
                    methods,
                });
            }
            SynItem::Fn(f) if is_pub(&f.vis) => {
                let (doc, line) = doc_and_line(&f.attrs, f.sig.ident.span());
                items.push(Item {
                    kind: "fn".into(),
                    source: None,
                    name: f.sig.ident.to_string(),
                    signature: fn_signature(&f.sig, &f.vis),
                    features,
                    doc,
                    line,
                    variants: Vec::new(),
                    fields: Vec::new(),
                    methods: Vec::new(),
                });
            }
            SynItem::Type(t) if is_pub(&t.vis) => {
                let (doc, line) = doc_and_line(&t.attrs, t.ident.span());
                items.push(Item {
                    kind: "type".into(),
                    source: None,
                    name: t.ident.to_string(),
                    signature: format!(
                        "{}type {}{} = {};",
                        vis_prefix(&t.vis),
                        t.ident,
                        generics_of(&t.generics),
                        render(t.ty.to_token_stream())
                    ),
                    features,
                    doc,
                    line,
                    variants: Vec::new(),
                    fields: Vec::new(),
                    methods: Vec::new(),
                });
            }
            SynItem::Const(c) if is_pub(&c.vis) => {
                let (doc, line) = doc_and_line(&c.attrs, c.ident.span());
                items.push(Item {
                    kind: "const".into(),
                    source: None,
                    name: c.ident.to_string(),
                    signature: format!(
                        "{}const {}: {};",
                        vis_prefix(&c.vis),
                        c.ident,
                        render(c.ty.to_token_stream())
                    ),
                    features,
                    doc,
                    line,
                    variants: Vec::new(),
                    fields: Vec::new(),
                    methods: Vec::new(),
                });
            }
            SynItem::Static(s) if is_pub(&s.vis) => {
                let (doc, line) = doc_and_line(&s.attrs, s.ident.span());
                items.push(Item {
                    kind: "static".into(),
                    source: None,
                    name: s.ident.to_string(),
                    signature: format!(
                        "{}static {}: {};",
                        vis_prefix(&s.vis),
                        s.ident,
                        render(s.ty.to_token_stream())
                    ),
                    features,
                    doc,
                    line,
                    variants: Vec::new(),
                    fields: Vec::new(),
                    methods: Vec::new(),
                });
            }
            SynItem::Impl(impl_block) => {
                if let Some(entry) = collect_methods(impl_block, &features) {
                    impl_blocks.push(entry);
                }
            }
            _ => {}
        }
    }

    for (type_name, methods) in impl_blocks {
        if let Some(item) = items.iter_mut().find(|i| i.name == type_name) {
            item.methods.extend(methods);
        } else if is_root {
            // Types defined in another file still need their methods kept.
            items.push(Item {
                kind: "impl".into(),
                source: None,
                name: type_name,
                signature: String::new(),
                doc: String::new(),
                features: inherited.to_vec(),
                line: 0,
                variants: Vec::new(),
                fields: Vec::new(),
                methods,
            });
        }
    }

    for item in &mut items {
        item.signature = normalize_signature(&item.signature);
        for method in &mut item.methods {
            method.signature = normalize_signature(&method.signature);
        }
    }

    Ok(Module {
        name: module_name,
        doc: module_doc(&file.attrs),
        features: inherited.to_vec(),
        source: source_path.to_string(),
        items,
    })
}

/// proc-macro2 injects spaces around generics and references; restore idiomatic spacing.
fn normalize_signature(text: &str) -> String {
    text.replace(" < ", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace("& ", "&")
        .replace(" ,", ",")
        .replace(" ;", ";")
        .replace(" :", ":")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace("  ", " ")
        .trim()
        .to_string()
}

fn item_attrs(item: &SynItem) -> &[Attribute] {
    match item {
        SynItem::Const(i) => &i.attrs,
        SynItem::Enum(i) => &i.attrs,
        SynItem::ExternCrate(i) => &i.attrs,
        SynItem::Fn(i) => &i.attrs,
        SynItem::ForeignMod(i) => &i.attrs,
        SynItem::Impl(i) => &i.attrs,
        SynItem::Macro(i) => &i.attrs,
        SynItem::Mod(i) => &i.attrs,
        SynItem::Static(i) => &i.attrs,
        SynItem::Struct(i) => &i.attrs,
        SynItem::Trait(i) => &i.attrs,
        SynItem::TraitAlias(i) => &i.attrs,
        SynItem::Type(i) => &i.attrs,
        SynItem::Union(i) => &i.attrs,
        SynItem::Use(i) => &i.attrs,
        SynItem::Verbatim(_) => &[],
        _ => &[],
    }
}

fn public_fields(fields: &syn::Fields) -> Vec<Field> {
    match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .filter(|f| is_pub(&f.vis))
            .filter_map(|f| {
                Some(Field {
                    name: f.ident.as_ref()?.to_string(),
                    ty: render(f.ty.to_token_stream()),
                    doc: item_doc(&f.attrs),
                    line: line_of(f.ident.as_ref()?.span()),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn module_path_for(source_path: &str) -> String {
    let trimmed = source_path
        .trim_start_matches("src/")
        .trim_end_matches(".rs");
    if trimmed == "lib" {
        String::new()
    } else {
        trimmed.replace('/', "::")
    }
}

pub fn child_modules(items: &[SynItem]) -> Vec<ChildModule> {
    let mut out = Vec::new();
    for item in items {
        let SynItem::Mod(module) = item else {
            continue;
        };
        out.push((
            module.ident.to_string(),
            cfg_features(&module.attrs),
            is_pub(&module.vis),
        ));
    }
    out
}

pub fn collect_sources(root: &std::path::Path) -> anyhow::Result<Vec<(String, String)>> {
    let src = root.join("src");
    let mut files = Vec::new();
    walk(&src, &src, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk(
    dir: &std::path::Path,
    src_root: &std::path::Path,
    out: &mut Vec<(String, String)>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, src_root, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let relative = path
                .strip_prefix(src_root)?
                .to_string_lossy()
                .replace('\\', "/");
            let content = std::fs::read_to_string(&path)?;
            out.push((format!("src/{relative}"), content));
        }
    }
    Ok(())
}

pub fn parse_files(files: &[(String, String)]) -> ParsedFiles {
    let mut map = HashMap::new();
    for (path, content) in files {
        if let Ok(file) = syn::parse_str::<syn::File>(content) {
            let children = child_modules(&file.items);
            map.insert(path.clone(), (file, children));
        }
    }
    map
}
