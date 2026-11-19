use proc_macro2::{Span, TokenStream, TokenTree};
use quote::ToTokens;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use syn::{
    visit::{self, Visit},
    Item, UseTree,
};

struct Inventory {
    root: PathBuf,
    modules: Vec<Value>,
    references: Vec<Value>,
    imports: Vec<Value>,
    module: Vec<String>,
    file: String,
    seen: BTreeSet<PathBuf>,
}
impl Inventory {
    fn reference(&mut self, path: Vec<String>, span: Span, kind: &str) {
        if path.is_empty() {
            return;
        }
        if path.len() == 1 && ["crate", "self", "super", "Self"].contains(&path[0].as_str()) {
            return;
        }
        self.references
            .push(json!({"module":self.module, "file":self.file,
            "path":path, "line":span.start().line, "kind":kind}));
    }
    fn imports(&mut self, tree: &UseTree, prefix: Vec<String>, public: bool) {
        match tree {
            UseTree::Path(p) => {
                let mut pfx = prefix;
                pfx.push(p.ident.to_string());
                self.imports(&p.tree, pfx, public);
            }
            UseTree::Group(g) => {
                for t in &g.items {
                    self.imports(t, prefix.clone(), public);
                }
            }
            UseTree::Name(n) => {
                let mut path = prefix;
                let alias = n.ident.to_string();
                if alias != "self" {
                    path.push(alias.clone());
                }
                let alias = if alias == "self" {
                    path.last().cloned().unwrap_or_default()
                } else {
                    alias
                };
                self.imports.push(json!({"module":self.module,"file":self.file,"path":path,
                    "alias":alias,"glob":false,"public":public,"line":n.ident.span().start().line}));
            }
            UseTree::Rename(r) => {
                let mut path = prefix;
                path.push(r.ident.to_string());
                self.imports.push(json!({"module":self.module,"file":self.file,"path":path,
                    "alias":r.rename.to_string(),"glob":false,"public":public,"line":r.ident.span().start().line}));
            }
            UseTree::Glob(g) => self
                .imports
                .push(json!({"module":self.module,"file":self.file,
                "path":prefix,"glob":true,"public":public,"line":g.star_token.span.start().line})),
        }
    }
    fn tokens(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                TokenTree::Group(g) => self.tokens(g.stream()),
                TokenTree::Ident(ident) => {
                    let span = ident.span();
                    let mut path = vec![ident.to_string()];
                    while i + 3 < tokens.len()
                        && matches!(&tokens[i+1], TokenTree::Punct(p) if p.as_char()==':')
                        && matches!(&tokens[i+2], TokenTree::Punct(p) if p.as_char()==':')
                    {
                        if let TokenTree::Ident(next) = &tokens[i + 3] {
                            path.push(next.to_string());
                            i += 3;
                        } else {
                            break;
                        }
                    }
                    self.reference(path, span, "macro token");
                }
                _ => {}
            }
            i += 1;
        }
    }
    fn items(&mut self, items: &[Item], directory: &Path) {
        let mut definitions = Vec::new();
        for item in items {
            let name = match item {
                Item::Struct(i) => Some(i.ident.to_string()),
                Item::Enum(i) => Some(i.ident.to_string()),
                Item::Union(i) => Some(i.ident.to_string()),
                Item::Trait(i) => Some(i.ident.to_string()),
                Item::Type(i) => Some(i.ident.to_string()),
                Item::Fn(i) => {
                    if i.attrs.iter().any(|a| {
                        a.path().segments.last().is_some_and(|s| {
                            [
                                "bind_resolved_func_detour",
                                "bind_resolved_request_function_detour",
                            ]
                            .contains(&s.ident.to_string().as_str())
                        })
                    }) {
                        if let Some(name) = i.sig.ident.to_string().strip_suffix("_detour") {
                            definitions.push(name.to_string());
                        }
                    }
                    Some(i.sig.ident.to_string())
                }
                Item::Const(i) => Some(i.ident.to_string()),
                Item::Static(i) => Some(i.ident.to_string()),
                Item::Macro(i) => {
                    let tokens: Vec<_> = i.mac.tokens.clone().into_iter().collect();
                    let macro_name = i.mac.path.segments.last().unwrap().ident.to_string();
                    if macro_name.starts_with("define_") && macro_name.ends_with("_resolver") {
                        if let Some(TokenTree::Ident(name)) = tokens.first() {
                            definitions.push(name.to_string());
                        }
                    }
                    for pair in tokens.windows(2) {
                        if matches!(&pair[0],TokenTree::Ident(i) if ["struct","enum","static"].contains(&i.to_string().as_str()))
                        {
                            if let TokenTree::Ident(name) = &pair[1] {
                                definitions.push(name.to_string());
                            }
                        }
                    }
                    i.ident.as_ref().map(ToString::to_string)
                }
                _ => None,
            };
            if let Some(name) = name {
                definitions.push(name);
            }
        }
        let macros: Vec<_> = items
            .iter()
            .filter_map(|item| match item {
                Item::Macro(i) => i.ident.as_ref().map(ToString::to_string),
                Item::Fn(i)
                    if i.attrs.iter().any(|a| {
                        ["proc_macro", "proc_macro_attribute", "proc_macro_derive"]
                            .iter()
                            .any(|n| a.path().is_ident(n))
                    }) =>
                {
                    Some(i.sig.ident.to_string())
                }
                _ => None,
            })
            .collect();
        self.modules.push(json!({"module":self.module,"file":self.file,"definitions":definitions,"macros":macros}));
        for item in items {
            if let Item::Mod(m) = item {
                self.reference(
                    vec!["self".into(), m.ident.to_string()],
                    m.ident.span(),
                    "module",
                );
                let old_module = self.module.clone();
                self.module.push(m.ident.to_string());
                if let Some((_, items)) = &m.content {
                    self.items(items, &directory.join(m.ident.to_string()));
                } else {
                    let path_attr = m.attrs.iter().find_map(|a| {
                        if !a.path().is_ident("path") {
                            return None;
                        }
                        if let syn::Meta::NameValue(v) = &a.meta {
                            if let syn::Expr::Lit(l) = &v.value {
                                if let syn::Lit::Str(s) = &l.lit {
                                    return Some(s.value());
                                }
                            }
                        }
                        None
                    });
                    let file = if let Some(path) = path_attr {
                        directory.join(path)
                    } else if directory.join(format!("{}.rs", m.ident)).exists() {
                        directory.join(format!("{}.rs", m.ident))
                    } else {
                        directory.join(m.ident.to_string()).join("mod.rs")
                    };
                    if file.exists() {
                        let old_file = self.file.clone();
                        self.load(&file);
                        self.file = old_file;
                    }
                }
                self.module = old_module;
            } else {
                self.visit_item(item);
            }
        }
    }
    fn load(&mut self, path: &Path) {
        if !self.seen.insert(path.to_path_buf()) {
            return;
        }
        self.file = path
            .strip_prefix(&self.root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let file = syn::parse_file(&std::fs::read_to_string(path).unwrap()).unwrap();
        let directory = if ["lib.rs", "main.rs", "mod.rs"]
            .contains(&path.file_name().unwrap().to_str().unwrap())
        {
            path.parent().unwrap().to_path_buf()
        } else {
            path.with_extension("")
        };
        self.items(&file.items, &directory);
    }
}
impl<'ast> Visit<'ast> for Inventory {
    fn visit_visibility(&mut self, _: &'ast syn::Visibility) {}
    fn visit_item_use(&mut self, i: &'ast syn::ItemUse) {
        self.imports(
            &i.tree,
            Vec::new(),
            !matches!(i.vis, syn::Visibility::Inherited),
        );
    }
    fn visit_path(&mut self, p: &'ast syn::Path) {
        self.reference(
            p.segments.iter().map(|s| s.ident.to_string()).collect(),
            p.segments.first().unwrap().ident.span(),
            "path",
        );
        visit::visit_path(self, p);
    }
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        self.visit_path(&m.path);
        self.tokens(m.tokens.clone());
    }
    fn visit_attribute(&mut self, a: &'ast syn::Attribute) {
        // Paths in native binding and macro attributes also name dependencies.
        self.tokens(a.meta.to_token_stream());
    }
}
fn main() {
    let root = std::fs::canonicalize(std::env::args().nth(1).unwrap_or(".".into())).unwrap();
    let mut inventory = Inventory {
        root: root.clone(),
        modules: Vec::new(),
        references: Vec::new(),
        imports: Vec::new(),
        module: Vec::new(),
        file: String::new(),
        seen: BTreeSet::new(),
    };
    for (name, entry) in [
        ("UnchainedPlugin", "src/lib.rs"),
        ("mock_server", "src/bin/mock_server.rs"),
        ("unchained_macros", "unchained-macros/src/lib.rs"),
        ("unchained_macros_impl", "unchained-macros/impl/src/lib.rs"),
        ("sleuth_macros", "sleuth_macros/src/lib.rs"),
    ] {
        inventory.module = vec![name.into()];
        inventory.load(&root.join(entry));
    }
    println!(
        "{}",
        json!({"modules":inventory.modules,"imports":inventory.imports,
        "references":inventory.references})
    );
}
