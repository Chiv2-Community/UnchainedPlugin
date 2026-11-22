//! Build-time native documentation shared by all Rustdoc/LSP clients.
pub mod cpp;
pub mod signature;

use anyhow::{Context, Result};
use proc_macro2::{Span, TokenTree};
use quote::ToTokens;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use syn::{spanned::Spanned, Attribute, Item};
use url::Url;

pub const CONFIGURATION_NOTE: &str =
    "Configure local source paths in .cargo/config.local.toml to view the implementation inline";
pub const NO_CPP_SOURCES: &str = "No cpp sources available";
const ARGON: &str = "https://github.com/Chiv2-Community/ArgonSDK/blob/main";
const UE: &str = "https://github.com/EpicGames/UnrealEngine/blob/4.25-plus";

#[derive(Clone)]
struct Source {
    url: String,
    local: Option<(PathBuf, PathBuf)>,
}

struct NativeType {
    name: String,
    label: &'static str,
    notes: String,
    source: Option<Source>,
}

struct Binding {
    target: String,
    overload: Option<String>,
    rpc_implementation: Option<String>,
    resolver: Option<String>,
    key: String,
    notes: String,
    source: Option<Source>,
    link: String,
    signature: String,
    parameter_types: Option<signature::BindingSignature>,
}

struct Detour {
    name: String,
    notes: String,
    link: String,
    declaration: String,
}

fn attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs.iter().find(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|part| part.ident == name)
    })
}

fn notes(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(value) = &attr.meta else {
                return None;
            };
            let syn::Expr::Lit(value) = &value.value else {
                return None;
            };
            let syn::Lit::Str(value) = &value.lit else {
                return None;
            };
            let value = value.value();
            Some(value.strip_prefix(' ').unwrap_or(&value).to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn text(source: &str, span: Span) -> &str {
    let offset = |position: proc_macro2::LineColumn| {
        let mut lines = source.split_inclusive('\n');
        let preceding = lines.by_ref()
            .take(position.line.saturating_sub(1))
            .map(str::len).sum::<usize>();
        let line = lines.next().unwrap_or("");
        preceding + line.char_indices().nth(position.column)
            .map_or(line.len(), |(byte, _)| byte)
    };
    &source[offset(span.start())..offset(span.end())]
}

fn file_link(path: &Path, line: usize) -> Result<String> {
    let url = Url::from_file_path(path)
        .map_err(|_| anyhow::anyhow!("Invalid source path: {}", path.display()))?;
    Ok(format!("{url}#L{line}"))
}

fn linked(name: &str, url: &str) -> String {
    format!("[`{name}`]({url})")
}

fn block(language: &str, code: &str) -> String {
    let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(3.max(longest + 1));
    format!("{fence}{language}\n{code}\n{fence}")
}

fn preview(code: &str) -> String {
    let code = code.split_whitespace().collect::<Vec<_>>().join(" ");
    if code.chars().count() <= 80 {
        code
    } else {
        code.chars().take(79).chain(['…']).collect()
    }
}

fn configured_root(
    manifest: &Path,
    name: &str,
    default: &str,
    local: &toml::Value,
) -> Result<String> {
    let entry = local.get("env").and_then(|env| env.get(name));
    let value = entry.and_then(|value| value.as_str().or_else(|| value.get("value")?.as_str()));
    let force = entry
        .and_then(|value| value.get("force"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let environment = std::env::var(name).ok();
    // Cargo injects the checked-in defaults into the build-script environment.
    let selected = if force {
        value.or(environment.as_deref()).unwrap_or(default)
    } else {
        environment
            .as_deref()
            .filter(|value| *value != default)
            .or(value)
            .unwrap_or(default)
    };
    if selected.starts_with("http://")
        || selected.starts_with("https://")
        || selected.starts_with("file://")
    {
        return Ok(selected.to_owned());
    }
    let path = PathBuf::from(selected);
    let relative = entry
        .and_then(|value| value.get("relative"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    let path = if relative { manifest.join(path) } else { path };
    anyhow::ensure!(path.is_absolute(), "{name} must be an absolute path or URL");
    Ok(Url::from_directory_path(path)
        .map_err(|_| anyhow::anyhow!("Invalid {name}"))?
        .to_string())
}

fn native_source(source: &str, roots: &BTreeMap<&str, String>) -> Result<Option<Source>> {
    let Some(url) = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .find_map(|line| {
            line.strip_prefix("//! Native source: <")?
                .split_once('>')
                .map(|pair| pair.0)
        })
    else {
        return Ok(None);
    };
    let parsed = Url::parse(url)?;
    let parts = parsed
        .path_segments()
        .map(|parts| parts.collect::<Vec<_>>())
        .unwrap_or_default();
    let key = match parts.as_slice() {
        ["Chiv2-Community", "ArgonSDK", "blob" | "tree", _, ..] => "ARGON_SDK_SOURCE_LOCATION",
        ["EpicGames", "UnrealEngine", "blob" | "tree", _, ..] => "UE_SOURCE_LOCATION",
        _ => {
            return Ok(Some(Source {
                url: url.to_owned(),
                local: None,
            }))
        }
    };
    if parsed.host_str() != Some("github.com") {
        return Ok(Some(Source {
            url: url.to_owned(),
            local: None,
        }));
    }
    let relative = parts[4..].join("/");
    let root = &roots[key];
    if root.starts_with("file://") {
        let root = Url::parse(root)?
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("Invalid local source root"))?;
        let header = PathBuf::from(relative);
        let url = file_link(&root.join(&header), 1)?;
        Ok(Some(Source {
            url,
            local: Some((root, header)),
        }))
    } else {
        Ok(Some(Source {
            url: format!("{}/{}", root.trim_end_matches('/'), relative),
            local: None,
        }))
    }
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            rust_files(&entry.path(), files)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "rs")
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn native_target(path: &syn::Path) -> String {
    path.segments
        .iter()
        .skip(path.segments.len().saturating_sub(2))
        .map(|part| part.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn native_implementation(
    source: Option<&Source>,
    target: &str,
    class: bool,
    watches: &mut BTreeSet<PathBuf>,
    sources: &mut cpp::SourceIndex,
    parameter_types: Option<&signature::BindingSignature>,
) -> Option<cpp::Implementation> {
    let (root, header) = source?.local.as_ref()?;
    watches.insert(root.join(header));
    if !class {
        watches.insert(cpp::module_directory(&root.join(header), root));
    }
    if class {
        sources.find_class(root, header, target).ok().flatten()
    } else {
        sources
            .find_implementation_for_binding(root, header, target, parameter_types)
            .ok()
            .flatten()
    }
}

fn append_native(
    docs: &mut String,
    implementation: Option<&cpp::Implementation>,
    source: Option<&Source>,
    title: &str,
    name: &str,
) -> Result<()> {
    docs.push_str("\n\n");
    if let Some(implementation) = implementation {
        docs.push_str(&format!(
            "{title}: {}\n\n{}",
            linked(name, &file_link(&implementation.path, implementation.line)?),
            block("cpp", &implementation.code)
        ));
    } else if source.is_some_and(|source| source.local.is_none()) {
        docs.push_str(CONFIGURATION_NOTE);
    } else {
        docs.push_str(NO_CPP_SOURCES);
    }
    Ok(())
}

fn write_document(
    directory: &Path,
    key: &str,
    docs: &str,
    written: &mut BTreeSet<String>,
) -> Result<()> {
    anyhow::ensure!(
        written.insert(key.to_owned()),
        "Duplicate native documentation key: {key}"
    );
    let path = directory.join(format!("{key}.md"));
    if fs::read_to_string(&path).ok().as_deref() != Some(docs) {
        fs::write(path, docs)?;
    }
    Ok(())
}

/// Generate standard Rustdoc includes before the consumer's macros expand.
pub fn generate(manifest: &Path, output: &Path) -> Result<()> {
    let local_path = manifest.join(".cargo/config.local.toml");
    let local: toml::Value = if local_path.exists() {
        fs::read_to_string(&local_path)?
            .parse()
            .context("Invalid .cargo/config.local.toml")?
    } else {
        toml::Value::Table(Default::default())
    };
    let roots = BTreeMap::from([
        (
            "ARGON_SDK_SOURCE_LOCATION",
            configured_root(manifest, "ARGON_SDK_SOURCE_LOCATION", ARGON, &local)?,
        ),
        (
            "UE_SOURCE_LOCATION",
            configured_root(manifest, "UE_SOURCE_LOCATION", UE, &local)?,
        ),
    ]);
    let mut watches = BTreeSet::from([
        manifest.join(".cargo"),
        manifest.join("src/Chivalry2"),
        manifest.join("src/unchained/patches"),
    ]);
    let mut files = Vec::new();
    rust_files(&manifest.join("src/Chivalry2"), &mut files)?;
    rust_files(&manifest.join("src/unchained/patches"), &mut files)?;
    let mut types = Vec::new();
    let mut bindings = Vec::new();
    let mut detours: BTreeMap<String, Vec<Detour>> = BTreeMap::new();
    let mut resolvers = BTreeMap::new();
    let mut sources = cpp::SourceIndex::new();
    for path in files {
        let source = fs::read_to_string(&path)?;
        let file =
            syn::parse_file(&source).with_context(|| format!("Cannot parse {}", path.display()))?;
        let file_source = native_source(&source, &roots)?;
        for item in file.items {
            let native_type = match &item {
                Item::Struct(item) => Some((&item.ident, &item.attrs, "class")),
                Item::Enum(item) => Some((&item.ident, &item.attrs, "enum")),
                Item::Union(item) => Some((&item.ident, &item.attrs, "union")),
                Item::Type(item) => Some((&item.ident, &item.attrs, "type")),
                _ => None,
            };
            if let Some((name, attrs, label)) = native_type {
                if attr(attrs, "runtime_bindings").is_some() {
                    let documentation = notes(attrs);
                    let item_source = documentation
                        .lines()
                        .filter(|line| line.starts_with("Native source: <"))
                        .map(|line| format!("//! {line}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    types.push(NativeType {
                        name: name.to_string(),
                        label,
                        notes: documentation
                            .lines()
                            .filter(|line| !line.starts_with("Native source: <"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        source: native_source(&item_source, &roots)?
                            .or_else(|| file_source.clone()),
                    });
                }
            }
            match item {
                Item::Macro(item)
                    if item.mac.path.segments.last().is_some_and(|part| {
                        matches!(
                            part.ident.to_string().as_str(),
                            "define_pattern_resolver"
                                | "define_custom_resolver"
                                | "define_offset_resolver"
                        )
                    }) =>
                {
                    let mut identifiers =
                        item.mac
                            .tokens
                            .clone()
                            .into_iter()
                            .filter_map(|token| match token {
                                TokenTree::Ident(name) => Some(name.to_string()),
                                _ => None,
                            });
                    if let Some(name) = identifiers.next() {
                        anyhow::ensure!(
                            resolvers
                                .insert(
                                    name.clone(),
                                    (
                                        file_link(&path, item.mac.span().start().line)?,
                                        preview(text(&source, item.span()))
                                    )
                                )
                                .is_none(),
                            "Duplicate resolver: {name}"
                        );
                    }
                }
                Item::Impl(item) => {
                    let owner = item.self_ty.to_token_stream().to_string().replace(' ', "");
                    let runtime_owner = attr(&item.attrs, "kismet_runtime_bindings")
                        .map(|attribute| attribute.parse_args::<syn::LitStr>()).transpose()?;
                    let annotated = runtime_owner.is_some() || attr(&item.attrs, "runtime_bindings").is_some();
                    let method_owner = match item.self_ty.as_ref() {
                        syn::Type::Path(path) => path.path.segments.last().unwrap().ident.to_string(),
                        _ => owner.clone(),
                    };
                    for method in item.items {
                        let syn::ImplItem::Fn(method) = method else {
                            continue;
                        };
                        let binding = attr(&method.attrs, "bind_resolved_func");
                        if binding.is_some() || annotated {
                            let documentation = notes(&method.attrs);
                            let resolver_name = if let Some(binding) = binding {
                                let resolver: syn::Path = binding.parse_args()?;
                                Some(resolver.segments.last().unwrap().ident.to_string())
                            } else if runtime_owner.is_some() {
                                Some(format!("{method_owner}__{}", method.sig.ident))
                            } else {
                                None
                            };
                            if let (Some(reflected_path), Some(resolver_name)) = (&runtime_owner, &resolver_name) {
                                resolvers.insert(resolver_name.clone(), (
                                    file_link(&path, method.sig.ident.span().start().line)?,
                                    format!("Reflection native body: {}::{}", reflected_path.value(), method.sig.ident),
                                ));
                            }
                            let native_owner = resolver_name.as_deref()
                                .and_then(|name| name.strip_suffix(&format!("__{}", method.sig.ident)))
                                .unwrap_or(&method_owner);
                            let key = resolver_name.as_ref().map_or_else(
                                || format!("method-{method_owner}__{}", method.sig.ident),
                                |name| format!("binding-{name}"),
                            );
                            let signature_span = if matches!(method.vis, syn::Visibility::Inherited)
                            {
                                method.sig.span()
                            } else {
                                method
                                    .vis
                                    .span()
                                    .join(method.sig.span())
                                    .unwrap_or(method.sig.span())
                            };
                            let parameter_types =
                                signature::BindingSignature::from_rust(&method.sig, native_owner);
                            bindings.push(Binding {
                                target: format!("{native_owner}::{}", method.sig.ident),
                                overload: documentation
                                    .lines()
                                    .find_map(|line| line.strip_prefix("Native overload: "))
                                    .map(|selector| selector.trim().trim_matches('`').to_owned()),
                                rpc_implementation: documentation
                                    .lines()
                                    .find_map(|line| {
                                        line.strip_prefix("Native RPC implementation: ")
                                    })
                                    .map(|target| target.trim().trim_matches('`').to_owned()),
                                resolver: resolver_name,
                                key,
                                notes: documentation
                                    .lines()
                                    .filter(|line| !line.starts_with("Native RPC implementation: "))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                source: file_source.clone(),
                                link: file_link(&path, method.sig.ident.span().start().line)?,
                                signature: format!("{};", text(&source, signature_span).trim()),
                                parameter_types,
                            });
                        }
                    }
                }
                Item::Fn(item) => {
                    if let Some(attribute) = attr(&item.attrs, "bind_func_detour") {
                        use syn::parse::Parser;
                        let args = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
                            .parse2(attribute.meta.require_list()?.tokens.clone())?;
                        let Some(syn::Expr::Path(target)) = args.first() else {
                            continue;
                        };
                        detours
                            .entry(native_target(&target.path))
                            .or_default()
                            .push(Detour {
                                name: item.sig.ident.to_string(),
                                notes: notes(&item.attrs),
                                link: file_link(&path, item.sig.ident.span().start().line)?,
                                declaration: text(&source, item.span()).to_owned(),
                            });
                    }
                }
                _ => {}
            }
        }
    }
    for binding in &bindings {
        if let Some((root, header)) = binding
            .source
            .as_ref()
            .and_then(|source| source.local.as_ref())
        {
            // Unavailable roots/headers retain the usual unavailable-source footer.
            let _ = sources.plan_implementation(root, header, &binding.target);
            if let Some(target) = &binding.rpc_implementation {
                let _ = sources.plan_implementation(root, header, target);
            }
        }
    }
    let directory = output.join("native-docs");
    fs::create_dir_all(&directory)?;
    let mut written = BTreeSet::new();
    for native in types {
        let mut docs = native.notes;
        let implementation = native_implementation(
            native.source.as_ref(),
            &native.name,
            true,
            &mut watches,
            &mut sources,
            None,
        );
        append_native(
            &mut docs,
            implementation.as_ref(),
            native.source.as_ref(),
            &format!("Native {} definition", native.label),
            &native.name,
        )?;
        write_document(
            &directory,
            &format!("type-{}", native.name),
            docs.trim_start(),
            &mut written,
        )?;
    }
    for binding in bindings {
        let source_link = binding
            .source
            .as_ref()
            .map(|source| source.url.as_str())
            .unwrap_or(&binding.link);
        let resolver = binding.resolver.as_ref().and_then(|name| resolvers.get(name));
        let resolution = if let Some((link, declaration)) = resolver {
            format!(
                "{}\n\n{}",
                linked(binding.resolver.as_ref().unwrap(), link),
                block("rust,ignore", declaration)
            )
        } else {
            binding.resolver.as_ref().map_or_else(String::new, |name| format!("`{name}` (upstream resolver)"))
        };
        let mut docs = if binding.resolver.is_some() {
            format!("Native binding to {}.\nResolved by {resolution}", linked(&binding.target, source_link))
        } else {
            format!("Native method {}.", linked(&binding.target, source_link))
        };
        let matching = detours
            .get(&binding.target)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for detour in matching {
            docs.push_str(&format!(
                "\n\nOverwritten by {}.\n\n{}",
                linked(&detour.name, &detour.link),
                block("rust,ignore", &detour.declaration)
            ));
        }
        if !matching.is_empty() {
            docs.push_str(
                "\n\nDetours apply only when installed and enabled; direct binding calls bypass them.",
            );
        }
        if !binding.notes.is_empty() {
            docs.push_str(&format!("\n\n{}", binding.notes));
        }
        let mut implementation = native_implementation(
            binding.source.as_ref(),
            &format!(
                "{}{}",
                binding.target,
                binding.overload.as_deref().unwrap_or("")
            ),
            false,
            &mut watches,
            &mut sources,
            binding.parameter_types.as_ref(),
        );
        let mut implementation_name = &binding.target;
        let mut implementation_title = "Native implementation";
        if implementation.is_none() {
            if let Some(target) = &binding.rpc_implementation {
                let related = native_implementation(
                    binding.source.as_ref(),
                    &format!("{target}{}", binding.overload.as_deref().unwrap_or("")),
                    false,
                    &mut watches,
                    &mut sources,
                    binding.parameter_types.as_ref(),
                );
                if related.is_some() {
                    implementation = related;
                    implementation_name = target;
                    implementation_title = "Related RPC implementation";
                }
            }
        }
        append_native(
            &mut docs,
            implementation.as_ref(),
            binding.source.as_ref(),
            implementation_title,
            implementation_name,
        )?;
        write_document(
            &directory,
            &binding.key,
            &docs,
            &mut written,
        )?;
        for detour in matching {
            let mut docs = format!("Overrides in-game calls to {}.\n\n{}\n\nInvoking `{}` in this project directly skips this detour.\n\nResolved by {resolution}",
                linked(&binding.target, &binding.link), block("rust,ignore", &binding.signature), binding.target);
            if !detour.notes.is_empty() {
                docs.push_str(&format!("\n\n{}", detour.notes));
            }
            append_native(
                &mut docs,
                implementation.as_ref(),
                binding.source.as_ref(),
                implementation_title,
                implementation_name,
            )?;
            write_document(
                &directory,
                &format!("detour-{}", detour.name),
                &docs,
                &mut written,
            )?;
        }
    }
    for path in watches {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    for key in ["ARGON_SDK_SOURCE_LOCATION", "UE_SOURCE_LOCATION"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    println!(
        "cargo:rustc-env=UNCHAINED_NATIVE_DOCS_DIR={}",
        directory.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_previews_are_at_most_eighty_characters() {
        let text = preview(&"λ".repeat(120));
        assert_eq!(text.chars().count(), 80);
        assert!(text.ends_with('…'));
        assert_eq!(preview("first\n    second\tthird"), "first second third");
        assert_eq!(preview(&"x".repeat(80)), "x".repeat(80));
    }

    #[test]
    fn detour_targets_keep_only_the_native_owner_and_method() {
        for path in [
            syn::parse_quote!(Native::Call),
            syn::parse_quote!(crate::Chivalry2::Native::Call),
            syn::parse_quote!(::engine::Native::Call),
        ] {
            assert_eq!(native_target(&path), "Native::Call");
        }
    }

    #[test]
    fn code_fences_preserve_embedded_backticks() {
        assert!(block("cpp", "const char* value = \"```\";").starts_with("````cpp"));
    }

    #[test]
    fn generated_detours_preserve_unicode_source_spans() {
        let root = std::env::temp_dir().join(format!("native-doc-unicode-{}", std::process::id()));
        fs::create_dir_all(root.join("src/Chivalry2")).unwrap();
        fs::create_dir_all(root.join("src/unchained/patches")).unwrap();
        fs::write(root.join("src/Chivalry2/Native.rs"),
            "impl Native { #[bind_resolved_func(Native__Call)] unsafe fn Call() {} }").unwrap();
        for payload in ["λ", "λλλλλλλλλλλλλλλλλλλλ", "日本語 /* λ */"] {
            let declaration = format!(
                "#[bind_func_detour(Native::Call)] unsafe fn unicode_detour() {{ /* λ */ trace!(\"{payload}\"); }}"
            );
            fs::write(root.join("src/unchained/patches/Test.rs"), &declaration).unwrap();
            generate(&root, &root.join("out")).unwrap();
            let docs = fs::read_to_string(root.join("out/native-docs/binding-Native__Call.md")).unwrap();
            assert!(docs.contains(&declaration), "{docs}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_source_links_do_not_request_local_configuration() {
        let mut docs = String::new();
        append_native(
            &mut docs,
            None,
            None,
            "Native implementation",
            "Native::Call",
        )
        .unwrap();
        assert!(docs.ends_with(NO_CPP_SOURCES));
        assert!(!docs.contains(CONFIGURATION_NOTE));
    }

    #[test]
    fn generates_native_type_binding_and_detour_docs_without_an_editor() {
        let root =
            std::env::temp_dir().join(format!("native-doc-generation-{}", std::process::id()));
        for path in [
            "src/Chivalry2",
            "src/unchained/patches",
            ".cargo",
            "cpp/Engine/Source/Test/Public",
            "cpp/Engine/Source/Test/Private",
        ] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        let cpp_root = Url::from_directory_path(root.join("cpp")).unwrap();
        fs::write(
            root.join(".cargo/config.local.toml"),
            format!("[env]\nUE_SOURCE_LOCATION = {{ value = \"{cpp_root}\", force = true }}\n"),
        )
        .unwrap();
        fs::write(root.join("cpp/Engine/Source/Test/Public/Native.h"),
            "class Native : public Base { public: const FString& Call() const; static void Typed(const FString& Address, float Timeout, Delegate Result) { Forward(Result); } private: int Value; }; class Reflected { public: static void Call(); private: static void Hidden(); };").unwrap();
        fs::write(
            root.join("cpp/Engine/Source/Test/Public/Faction.h"),
            "enum class Faction : unsigned char { First, Second };",
        )
        .unwrap();
        fs::write(
            root.join("cpp/Engine/Source/Test/Private/Native.cpp"),
            "const FString& Native::Call() const { return Value; }\nconst FString& Native::Call(int Index) const { return Other; }\nvoid Native::Rpc_Implementation() { HandleMessage(); }\nvoid Native::Plain_Implementation() { NotTheBoundFunction(); }\nvoid Native::Typed(const FString& Address, float Timeout, Callback Result) { Send(Result); }\nvoid Native::Helper() const { NoBinding(); }\nvoid Native::GlobalHelper() const { FromGlobal(); }\nvoid Native::PrivateGlobal() const { PrivateSource(); }\nvoid Reflected::Call() { ReflectedSource(); }\nvoid Reflected::Hidden() { HiddenSource(); }",
        )
        .unwrap();
        fs::write(root.join("src/Chivalry2/Native.rs"), r#"//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Test/Public/Native.h>.
#[runtime_bindings]
pub struct Native;
/// Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Test/Public/Faction.h>.
/// Faction notes.
#[runtime_bindings]
pub enum Faction { First, Second }
define_pattern_resolver!(Native__Call, ["40 53 48 83 EC 20 48 8B D9 E8 ?? ?? ?? ?? 48 8B"]);
define_pattern_resolver!(Native__Other, ["40 53"]);
define_pattern_resolver!(Native__Rpc, ["40 54"]);
define_pattern_resolver!(Native__Plain, ["40 55"]);
define_pattern_resolver!(Native__Typed, ["40 56"]);
define_offset_resolver!(Native__Provided);
#[runtime_bindings]
impl Native {
    /// # Safety
    /// Keep the receiver alive.
    /// Native overload: `() const`
    #[bind_resolved_func(Native__Call)]
    pub unsafe fn Call(&self) -> &FString {}
    #[bind_resolved_func(Native__Other)]
    pub unsafe fn Other(&self) {}
    /// Calls a generated RPC wrapper.
    /// Native RPC implementation: `Native::Rpc_Implementation`
    #[bind_resolved_func(Native__Rpc)]
    pub unsafe fn Rpc(&self) {}
    #[bind_resolved_func(Native__Plain)]
    pub unsafe fn Plain(&self) {}
    #[bind_resolved_func(Native__Typed)]
    pub unsafe fn Typed(address: *const FString, timeout: f32, #[native_by_value] callback: *mut Callback) {}
    #[bind_resolved_func(Native__Provided)]
    pub unsafe fn Provided(&self) {}
    /// Helper notes.
    fn Helper(&self) {}
}
#[runtime_bindings]
impl Native {
    pub unsafe fn GlobalHelper(&self) {}
    fn PrivateGlobal(&self) {}
}
#[kismet_runtime_bindings("/Script/Fixture.Reflected")]
impl Reflected {
    /// Reflected safety notes.
    pub unsafe fn Call() {}
    unsafe fn Hidden() {}
}
impl Unannotated {
    fn Ignored(&self) {}
}
"#).unwrap();
        fs::write(
            root.join("src/unchained/patches/Test.rs"),
            "#[bind_func_detour(Native::Call)]\nunsafe fn call_detour() { replacement(); }\n#[bind_func_detour(Native::Rpc)]\nunsafe fn rpc_detour() {}",
        )
        .unwrap();
        generate(&root, &root.join("out")).unwrap();
        let directory = root.join("out/native-docs");
        let class = fs::read_to_string(directory.join("type-Native.md")).unwrap();
        assert!(class.starts_with("Native class definition:"));
        assert!(!class.contains("Native class:"));
        assert!(class.contains("class Native : public Base"));
        assert!(class.contains("private: int Value;"));
        let faction = fs::read_to_string(directory.join("type-Faction.md")).unwrap();
        assert!(faction.contains("enum class Faction : unsigned char { First, Second };"));
        assert!(faction.contains("Faction notes."));
        assert!(!faction.contains("Native enum:"));
        assert!(!faction.contains("Native source:"));
        assert!(!faction.contains("Native.h"));
        let binding = fs::read_to_string(directory.join("binding-Native__Call.md")).unwrap();
        assert!(binding.contains("Overwritten by [`call_detour`](file:"));
        assert!(binding.contains(".\nResolved by "));
        assert!(!binding.contains(".\n\nResolved by "));
        assert!(binding.contains("Detours apply only when installed and enabled;"));
        assert!(binding.contains("Keep the receiver alive."));
        assert!(binding.contains("const FString& Native::Call() const { return Value; }"));
        assert!(!binding.contains(CONFIGURATION_NOTE));
        let resolver_preview = binding
            .split("```rust,ignore\n")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        assert_eq!(resolver_preview.chars().count(), 80);
        assert!(resolver_preview.ends_with('…'));
        assert!(binding.contains(
            "#[bind_func_detour(Native::Call)]\nunsafe fn call_detour() { replacement(); }"
        ));
        let other = fs::read_to_string(directory.join("binding-Native__Other.md")).unwrap();
        assert!(!other.contains("Overwritten by"));
        assert!(!other.contains("Detours apply only when"));
        assert!(other.ends_with(NO_CPP_SOURCES));
        let rpc = fs::read_to_string(directory.join("binding-Native__Rpc.md")).unwrap();
        assert!(rpc.starts_with("Native binding to [`Native::Rpc`]"));
        assert!(rpc.contains("Calls a generated RPC wrapper."));
        assert!(!rpc.contains("Native RPC implementation:"));
        assert!(rpc.contains("Related RPC implementation: [`Native::Rpc_Implementation`]"));
        assert!(rpc.contains("void Native::Rpc_Implementation() { HandleMessage(); }"));
        assert!(!rpc.contains(NO_CPP_SOURCES));
        let rpc_detour = fs::read_to_string(directory.join("detour-rpc_detour.md")).unwrap();
        assert!(rpc_detour.starts_with("Overrides in-game calls to [`Native::Rpc`]"));
        assert!(rpc_detour.contains("Related RPC implementation:"));
        assert!(rpc_detour.contains("HandleMessage();"));
        let plain = fs::read_to_string(directory.join("binding-Native__Plain.md")).unwrap();
        assert!(plain.ends_with(NO_CPP_SOURCES));
        assert!(!plain.contains("NotTheBoundFunction"));
        let typed = fs::read_to_string(directory.join("binding-Native__Typed.md")).unwrap();
        let provided = fs::read_to_string(directory.join("binding-Native__Provided.md")).unwrap();
        assert!(provided.contains("define_offset_resolver!(Native__Provided)"));
        assert!(!provided.contains("upstream resolver"));
        assert!(typed.contains("void Native::Typed(const FString& Address, float Timeout, Callback Result) { Send(Result); }"));
        assert!(!typed.contains("Native overload:"));
        assert!(!typed.contains("Forward(Result)"));
        for (name, source) in [
            ("Helper", "NoBinding();"),
            ("GlobalHelper", "FromGlobal();"),
            ("PrivateGlobal", "PrivateSource();"),
        ] {
            let method = fs::read_to_string(directory.join(format!("method-Native__{name}.md"))).unwrap();
            assert!(method.starts_with(&format!("Native method [`Native::{name}`]")));
            assert!(method.contains(source));
            assert!(!method.contains("Resolved by"));
            assert!(!method.contains("upstream resolver"));
        }
        let helper = fs::read_to_string(directory.join("method-Native__Helper.md")).unwrap();
        assert!(helper.contains("Helper notes."));
        assert!(!directory.join("method-Unannotated__Ignored.md").exists());
        for (name, source) in [("Call", "ReflectedSource();"), ("Hidden", "HiddenSource();")] {
            let method = fs::read_to_string(directory.join(format!("binding-Reflected__{name}.md"))).unwrap();
            assert!(method.contains(&format!("Reflection native body: /Script/Fixture.Reflected::{name}")));
            assert!(method.contains(source));
            assert!(method.contains("Native.cpp#L"));
        }
        let reflected = fs::read_to_string(directory.join("binding-Reflected__Call.md")).unwrap();
        assert!(reflected.contains("Reflected safety notes."));
        let cpp = root.join("cpp/Engine/Source/Test/Private/Native.cpp");
        fs::write(
            &cpp,
            format!(
                "{}\nvoid Native::Rpc() {{ ProcessEvent(); }}",
                fs::read_to_string(&cpp).unwrap()
            ),
        )
        .unwrap();
        generate(&root, &root.join("out")).unwrap();
        let rpc = fs::read_to_string(directory.join("binding-Native__Rpc.md")).unwrap();
        assert!(rpc.contains("Native implementation: [`Native::Rpc`]"));
        assert!(rpc.contains("void Native::Rpc() { ProcessEvent(); }"));
        assert!(!rpc.contains("Related RPC implementation"));
        let detour = fs::read_to_string(directory.join("detour-call_detour.md")).unwrap();
        assert!(detour.contains("pub unsafe fn Call(&self) -> &FString;"));
        assert!(detour.contains("Native implementation:"));
        let rust = root.join("src/Chivalry2/Native.rs");
        let aliased = fs::read_to_string(&rust)
            .unwrap()
            .replace("pub struct Native;", "pub type Native = TArray<TCHAR>;")
            .replace("impl Native {", "impl TArray<TCHAR> {");
        fs::write(&rust, aliased).unwrap();
        generate(&root, &root.join("out")).unwrap();
        let binding = fs::read_to_string(directory.join("binding-Native__Call.md")).unwrap();
        assert!(binding.contains("Native binding to [`Native::Call`]"));
        assert!(binding.contains("Overwritten by [`call_detour`]"));
        assert!(binding.contains("const FString& Native::Call() const"));
        fs::write(
            root.join("cpp/Engine/Source/Test/Private/Native.cpp"),
            "// No function body.",
        )
        .unwrap();
        generate(&root, &root.join("out")).unwrap();
        let binding = fs::read_to_string(directory.join("binding-Native__Call.md")).unwrap();
        assert!(binding.ends_with(NO_CPP_SOURCES));
        let rpc = fs::read_to_string(directory.join("binding-Native__Rpc.md")).unwrap();
        assert!(rpc.ends_with(NO_CPP_SOURCES));
        let detour = fs::read_to_string(directory.join("detour-call_detour.md")).unwrap();
        assert!(detour.ends_with(NO_CPP_SOURCES));
        fs::write(
            root.join(".cargo/config.local.toml"),
            format!("[env]\nUE_SOURCE_LOCATION = {{ value = \"{UE}\", force = true }}\n"),
        )
        .unwrap();
        generate(&root, &root.join("out")).unwrap();
        let class = fs::read_to_string(directory.join("type-Native.md")).unwrap();
        assert_eq!(class, CONFIGURATION_NOTE);
        fs::write(root.join("src/unchained/patches/Test.rs"), "").unwrap();
        generate(&root, &root.join("out")).unwrap();
        let binding = fs::read_to_string(directory.join("binding-Native__Call.md")).unwrap();
        assert!(!binding.contains("Overwritten by"));
        assert!(!binding.contains("Detours apply only when"));
        fs::remove_dir_all(root).unwrap();
    }
}
