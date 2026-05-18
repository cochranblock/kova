// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Sonnet 4.6
//! carve_static — static corpus carver.
//!
//! Walks --input-dir (default `/mnt/data/crates/`), extracts .rs files from
//! each .crate archive in memory, syn-parses them, and emits labeled JSONL
//! per subatomic model to --output-dir (default `/mnt/data/training/static/`).
//!
//! Models emitted (one .jsonl file each):
//!   async_detector  — sync=0, async=1
//!   arg_count       — 0/1/2/3/4+ (labels 0-4)
//!   return_type     — void=0, Result=1, Option=2, primitive=3, custom=4
//!   visibility      — private=0, pub=1, pub_crate=2
//!   self_receiver   — none=0, ref_self=1, ref_mut_self=2, owned_self=3
//!   field_count     — 1-3=0, 4-8=1, 9+=2
//!   variant_count   — 2-4=0, 5-10=1, 11+=2
//!   error_enum      — no=0, yes=1
//!   method_count    — 0-2=0, 3-5=1, 6+=2
//!   has_debug       — no=0, yes=1
//!   has_clone       — no=0, yes=1
//!   has_serialize   — no=0, yes=1
//!   has_deserialize — no=0, yes=1
//!   has_default     — no=0, yes=1
//!   has_partial_eq  — no=0, yes=1
//!
//! Run:
//!   cargo run --release --bin carve-static --features carve -- --max-crates 1000

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use flate2::read::GzDecoder;
use serde_json::json;
use syn::visit::Visit;

#[derive(Parser)]
#[command(about = "Static corpus carver — syn-parse .crate files → labeled JSONL")]
struct Args {
    #[arg(long, default_value = "/mnt/data/crates")]
    input_dir: PathBuf,
    #[arg(long, default_value = "/mnt/data/training/static")]
    output_dir: PathBuf,
    /// Stop after processing N crates (0 = all)
    #[arg(long, default_value = "0")]
    max_crates: usize,
    /// Print progress every N crates
    #[arg(long, default_value = "5000")]
    progress: usize,
}

// ── Class labels ────────────────────────────────────────────────────────────

const ARG_COUNT_CLASSES: &[&str] = &["0", "1", "2", "3", "4+"];
const FIELD_COUNT_CLASSES: &[&str] = &["1-3", "4-8", "9+"];
const VARIANT_COUNT_CLASSES: &[&str] = &["2-4", "5-10", "11+"];
const SELF_RECV_CLASSES: &[&str] = &["none", "ref_self", "ref_mut_self", "owned_self"];
const METHOD_COUNT_CLASSES: &[&str] = &["0-2", "3-5", "6+"];

const ALL_MODELS: &[&str] = &[
    "async_detector",
    "arg_count",
    "return_type",
    "visibility",
    "self_receiver",
    "field_count",
    "variant_count",
    "error_enum",
    "method_count",
    "has_debug",
    "has_clone",
    "has_serialize",
    "has_deserialize",
    "has_default",
    "has_partial_eq",
];

// ── Example ─────────────────────────────────────────────────────────────────

struct Ex {
    model: &'static str,
    text: String,
    label: u8,
    class: &'static str,
}

// ── Visitor / collector ──────────────────────────────────────────────────────

struct Collector {
    examples: Vec<Ex>,
}

impl Collector {
    fn push(&mut self, model: &'static str, text: String, label: u8, class: &'static str) {
        if text.len() < 4 || text.len() > 512 {
            return;
        }
        self.examples.push(Ex { model, text, label, class });
    }

    fn process_fn_sig(&mut self, sig: &syn::Signature, vis: &syn::Visibility, is_method: bool) {
        let name = sig.ident.to_string();
        let is_async = sig.asyncness.is_some();

        let arg_types: Vec<String> = sig.inputs.iter().filter_map(|a| match a {
            syn::FnArg::Typed(pt) => Some(ty_str(&pt.ty)),
            syn::FnArg::Receiver(_) => None,
        }).collect();
        let arg_count = arg_types.len();

        let self_label: u8 = sig.inputs.iter().find_map(|a| {
            if let syn::FnArg::Receiver(r) = a {
                Some(match (r.reference.is_some(), r.mutability.is_some()) {
                    (true, false) => 1,
                    (true, true) => 2,
                    _ => 3,
                })
            } else {
                None
            }
        }).unwrap_or(0);

        let (ret_label, ret_class) = match &sig.output {
            syn::ReturnType::Default => (0u8, "void"),
            syn::ReturnType::Type(_, ty) => ret_type_label(ty),
        };
        let ret_str = match &sig.output {
            syn::ReturnType::Default => String::new(),
            syn::ReturnType::Type(_, ty) => format!(" -> {}", ty_str(ty)),
        };

        let async_kw = if is_async { "async fn " } else { "fn " };
        let text = format!("{async_kw}{name}({}){ret_str}", arg_types.join(", "));

        self.push(
            "async_detector",
            text.clone(),
            is_async as u8,
            if is_async { "async" } else { "sync" },
        );

        let arg_label = match arg_count {
            0 => 0u8,
            1 => 1,
            2 => 2,
            3 => 3,
            _ => 4,
        };
        self.push("arg_count", text.clone(), arg_label, ARG_COUNT_CLASSES[arg_label as usize]);
        self.push("return_type", text.clone(), ret_label, ret_class);

        if !is_method {
            let (vl, vc) = vis_label(vis);
            self.push("visibility", format!("fn {name}{ret_str}"), vl, vc);
        } else {
            self.push(
                "self_receiver",
                format!("fn {name}{ret_str}"),
                self_label,
                SELF_RECV_CLASSES[self_label as usize],
            );
        }
    }

    fn process_struct(&mut self, s: &syn::ItemStruct) {
        let name = s.ident.to_string();
        let fields: Vec<String> = match &s.fields {
            syn::Fields::Named(nf) => nf.named.iter().map(|f| {
                let fname = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                format!("{fname}: {}", ty_str(&f.ty))
            }).collect(),
            syn::Fields::Unnamed(uf) => uf.unnamed.iter().map(|f| ty_str(&f.ty)).collect(),
            syn::Fields::Unit => vec![],
        };
        let fc = fields.len();
        let text = format!("struct {name} {{ {} }}", fields.join(", "));

        if fc > 0 {
            let fl: u8 = if fc <= 3 { 0 } else if fc <= 8 { 1 } else { 2 };
            self.push("field_count", text.clone(), fl, FIELD_COUNT_CLASSES[fl as usize]);
        }

        let (vl, vc) = vis_label(&s.vis);
        self.push("visibility", format!("struct {name}"), vl, vc);

        let derives = collect_derives(&s.attrs);
        emit_derives(self, &text, &derives);
    }

    fn process_enum(&mut self, e: &syn::ItemEnum) {
        let name = e.ident.to_string();
        let vc = e.variants.len();
        let variants: Vec<String> = e.variants.iter().map(|v| v.ident.to_string()).collect();
        let text = format!("enum {name} {{ {} }}", variants.join(", "));

        if vc >= 2 {
            let vl: u8 = if vc <= 4 { 0 } else if vc <= 10 { 1 } else { 2 };
            self.push("variant_count", text.clone(), vl, VARIANT_COUNT_CLASSES[vl as usize]);
        }

        let is_error = name.ends_with("Error") || name.ends_with("Err") || name == "Error";
        self.push(
            "error_enum",
            text.clone(),
            is_error as u8,
            if is_error { "yes" } else { "no" },
        );

        let (vl, vc_v) = vis_label(&e.vis);
        self.push("visibility", format!("enum {name}"), vl, vc_v);

        let derives = collect_derives(&e.attrs);
        emit_derives(self, &text, &derives);
    }

    fn process_trait(&mut self, t: &syn::ItemTrait) {
        let name = t.ident.to_string();
        let mc = t.items.iter().filter(|i| matches!(i, syn::TraitItem::Fn(_))).count();
        let ml: u8 = if mc <= 2 { 0 } else if mc <= 5 { 1 } else { 2 };
        self.push(
            "method_count",
            format!("trait {name} methods={mc}"),
            ml,
            METHOD_COUNT_CLASSES[ml as usize],
        );
    }
}

impl<'ast> Visit<'ast> for Collector {
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.process_fn_sig(&f.sig, &f.vis, false);
        syn::visit::visit_item_fn(self, f);
    }

    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        self.process_fn_sig(&f.sig, &f.vis, true);
        syn::visit::visit_impl_item_fn(self, f);
    }

    fn visit_item_struct(&mut self, s: &'ast syn::ItemStruct) {
        self.process_struct(s);
        syn::visit::visit_item_struct(self, s);
    }

    fn visit_item_enum(&mut self, e: &'ast syn::ItemEnum) {
        self.process_enum(e);
        syn::visit::visit_item_enum(self, e);
    }

    fn visit_item_trait(&mut self, t: &'ast syn::ItemTrait) {
        self.process_trait(t);
        syn::visit::visit_item_trait(self, t);
    }
}

// ── AST helpers ──────────────────────────────────────────────────────────────

fn ty_str(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => p
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        syn::Type::Reference(r) => {
            let m = if r.mutability.is_some() { "mut " } else { "" };
            format!("&{m}{}", ty_str(&r.elem))
        }
        syn::Type::Slice(s) => format!("[{}]", ty_str(&s.elem)),
        syn::Type::Array(a) => format!("[{}]", ty_str(&a.elem)),
        syn::Type::Tuple(t) => {
            if t.elems.is_empty() {
                "()".to_string()
            } else {
                let elems: Vec<_> = t.elems.iter().map(ty_str).collect();
                format!("({})", elems.join(", "))
            }
        }
        syn::Type::ImplTrait(_) | syn::Type::TraitObject(_) => "dyn".to_string(),
        _ => "_".to_string(),
    }
}

fn ret_type_label(ty: &syn::Type) -> (u8, &'static str) {
    match ty {
        syn::Type::Path(p) => {
            let name = p.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
            match name.as_str() {
                "Result" => (1, "Result"),
                "Option" => (2, "Option"),
                "bool" | "i8" | "u8" | "i16" | "u16" | "i32" | "u32" | "i64" | "u64"
                | "i128" | "u128" | "isize" | "usize" | "f32" | "f64" | "String" | "str"
                | "char" | "CString" | "OsString" => (3, "primitive"),
                _ => (4, "custom"),
            }
        }
        syn::Type::Tuple(t) if t.elems.is_empty() => (0, "void"),
        syn::Type::Reference(_) => (3, "primitive"),
        _ => (4, "custom"),
    }
}

fn vis_label(vis: &syn::Visibility) -> (u8, &'static str) {
    match vis {
        syn::Visibility::Public(_) => (1, "pub"),
        syn::Visibility::Restricted(r) => {
            if r.path.is_ident("crate") {
                (2, "pub_crate")
            } else {
                (1, "pub")
            }
        }
        syn::Visibility::Inherited => (0, "private"),
    }
}

fn collect_derives(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut derives = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("derive") {
            let _ = attr.parse_nested_meta(|meta| {
                if let Some(ident) = meta.path.get_ident() {
                    derives.push(ident.to_string());
                }
                Ok(())
            });
        }
    }
    derives
}

fn emit_derives(col: &mut Collector, text: &str, derives: &[String]) {
    const TRACKED: &[(&str, &'static str)] = &[
        ("Debug", "has_debug"),
        ("Clone", "has_clone"),
        ("Serialize", "has_serialize"),
        ("Deserialize", "has_deserialize"),
        ("Default", "has_default"),
        ("PartialEq", "has_partial_eq"),
    ];
    for (derive_name, model) in TRACKED {
        let present = derives.iter().any(|d| d == derive_name);
        col.push(
            model,
            text.to_owned(),
            present as u8,
            if present { "yes" } else { "no" },
        );
    }
}

// ── Corpus walking ───────────────────────────────────────────────────────────

fn walk_crates(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_crates(&p, out);
        } else if p.extension().map(|e| e == "crate").unwrap_or(false) {
            out.push(p);
        }
    }
}

fn process_crate(path: &Path) -> Vec<Ex> {
    let Ok(file) = File::open(path) else { return vec![] };
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let Ok(entries) = archive.entries() else { return vec![] };

    let mut examples = Vec::new();
    for entry in entries {
        let mut entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let is_rs = entry
            .path()
            .ok()
            .and_then(|p| p.extension().map(|e| e == "rs"))
            .unwrap_or(false);
        if !is_rs {
            continue;
        }

        let mut content = String::new();
        if entry.read_to_string(&mut content).is_err() {
            continue;
        }
        // Skip generated / very large files.
        if content.len() > 512 * 1024 {
            continue;
        }

        let Ok(ast) = syn::parse_file(&content) else { continue };

        let mut col = Collector { examples: Vec::new() };
        col.visit_file(&ast);
        examples.extend(col.examples.drain(..));
    }
    examples
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();

    eprintln!("[carve] scanning {}...", args.input_dir.display());
    let mut crate_paths: Vec<PathBuf> = Vec::new();
    walk_crates(&args.input_dir, &mut crate_paths);
    eprintln!("[carve] found {} .crate files", crate_paths.len());

    let limit = if args.max_crates == 0 {
        crate_paths.len()
    } else {
        args.max_crates.min(crate_paths.len())
    };
    let crate_paths = &crate_paths[..limit];

    fs::create_dir_all(&args.output_dir).expect("create output dir");

    let mut writers: HashMap<&'static str, BufWriter<File>> = HashMap::new();
    let mut counts: HashMap<&'static str, u64> = HashMap::new();
    for model in ALL_MODELS {
        let path = args.output_dir.join(format!("{model}.jsonl"));
        let file = File::create(&path).expect("create model file");
        writers.insert(model, BufWriter::new(file));
        counts.insert(model, 0);
    }

    let total = crate_paths.len();
    let mut parsed_ok = 0u64;
    let mut total_examples = 0u64;

    for (i, crate_path) in crate_paths.iter().enumerate() {
        let examples = process_crate(crate_path);
        if !examples.is_empty() {
            parsed_ok += 1;
        }
        for ex in examples {
            if let Some(w) = writers.get_mut(ex.model) {
                let line = json!({ "text": ex.text, "label": ex.label, "class": ex.class });
                let _ = writeln!(w, "{line}");
                *counts.entry(ex.model).or_insert(0) += 1;
                total_examples += 1;
            }
        }
        if args.progress > 0 && (i + 1) % args.progress == 0 {
            eprintln!("[carve] {}/{} crates processed, {} examples so far", i + 1, total, total_examples);
        }
    }

    for w in writers.values_mut() {
        let _ = w.flush();
    }

    eprintln!("[carve] done: {total} crates ({parsed_ok} had parseable Rust)");
    eprintln!("[carve] {total_examples} total examples → {}", args.output_dir.display());
    let mut count_vec: Vec<_> = counts.iter().collect();
    count_vec.sort_by_key(|(k, _)| *k);
    for (model, count) in count_vec {
        eprintln!("  {model}: {count}");
    }
}
