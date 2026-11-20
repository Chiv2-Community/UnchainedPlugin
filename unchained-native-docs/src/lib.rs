//! Build-time native documentation shared by all Rustdoc/LSP clients.
pub mod cpp;

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

struct Source {
    url: String,
    local: Option<(PathBuf, PathBuf)>,
}

struct NativeType {
    name: String,
    label: &'static str,
    notes: String,
    source: Option<Source>,
    link: String,
}

struct Binding {
    target: String,
    resolver: String,
    request: bool,
    notes: String,
    source: Option<Source>,
    link: String,
    signature: String,
}

struct Detour {
    name: String,
    target: String,
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
        source
            .split_inclusive('\n')
            .take(position.line.saturating_sub(1))
            .map(str::len)
            .sum::<usize>()
            + position.column
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
        .rev()
        .take(2)
        .map(|part| part.ident.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("::")
}

fn native_implementation(
    source: Option<&Source>,
    target: &str,
    class: bool,
    watches: &mut BTreeSet<PathBuf>,
) -> Option<cpp::Implementation> {
    let (root, header) = source?.local.as_ref()?;
    watches.insert(root.join(header));
    if !class {
        watches.insert(cpp::module_directory(&root.join(header), root));
    }
    if class {
        cpp::find_class(root, header, target).ok().flatten()
    } else {
        cpp::find_implementation(root, header, target)
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
    let mut detours = Vec::new();
    let mut resolvers = BTreeMap::new();
    for path in files {
        let source = fs::read_to_string(&path)?;
        let file =
            syn::parse_file(&source).with_context(|| format!("Cannot parse {}", path.display()))?;
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
                    types.push(NativeType {
                        name: name.to_string(),
                        label,
                        notes: notes(attrs),
                        source: native_source(&source, &roots)?,
                        link: file_link(&path, name.span().start().line)?,
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
                                | "define_single_platform_pattern_resolver"
                        )
                    }) =>
                {
                    let identifiers = item
                        .mac
                        .tokens
                        .clone()
                        .into_iter()
                        .filter_map(|token| match token {
                            TokenTree::Ident(name) => Some(name.to_string()),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let index = usize::from(
                        item.mac.path.segments.last().unwrap().ident
                            == "define_single_platform_pattern_resolver",
                    );
                    if let Some(name) = identifiers.get(index) {
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
                    for method in item.items {
                        let syn::ImplItem::Fn(method) = method else {
                            continue;
                        };
                        let binding = attr(&method.attrs, "bind_resolved_func")
                            .or_else(|| attr(&method.attrs, "bind_resolved_request_function"));
                        if let Some(binding) = binding {
                            let resolver: syn::Path = binding.parse_args()?;
                            let resolver_name = resolver.segments.last().unwrap().ident.to_string();
                            let native_owner = resolver_name
                                .strip_suffix(&format!("__{}", method.sig.ident))
                                .unwrap_or(&owner);
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
                            bindings.push(Binding {
                                target: format!("{native_owner}::{}", method.sig.ident),
                                resolver: resolver_name,
                                request: binding.path().segments.last().unwrap().ident
                                    == "bind_resolved_request_function",
                                notes: notes(&method.attrs),
                                source: native_source(&source, &roots)?,
                                link: file_link(&path, method.sig.ident.span().start().line)?,
                                signature: format!("{};", text(&source, signature_span).trim()),
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
                        detours.push(Detour {
                            name: item.sig.ident.to_string(),
                            target: native_target(&target.path),
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
    let directory = output.join("native-docs");
    fs::create_dir_all(&directory)?;
    let mut written = BTreeSet::new();
    for native in types {
        let link = native
            .source
            .as_ref()
            .map(|source| source.url.as_str())
            .unwrap_or(&native.link);
        let mut docs = format!("Native {}: {}.", native.label, linked(&native.name, link));
        if !native.notes.is_empty() {
            docs.push_str(&format!("\n\n{}", native.notes));
        }
        let implementation =
            native_implementation(native.source.as_ref(), &native.name, true, &mut watches);
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
            &docs,
            &mut written,
        )?;
    }
    for binding in bindings {
        let source_link = binding
            .source
            .as_ref()
            .map(|source| source.url.as_str())
            .unwrap_or(&binding.link);
        let resolver = resolvers.get(&binding.resolver);
        let resolution = if binding.request {
            format!(
                "`{}` (offset-registry key)\n\n{}",
                binding.resolver,
                block(
                    "rust,ignore",
                    &format!(
                        "{} (offset-registry key; no resolver declaration)",
                        binding.resolver
                    )
                )
            )
        } else if let Some((link, declaration)) = resolver {
            format!(
                "{}\n\n{}",
                linked(&binding.resolver, link),
                block("rust,ignore", declaration)
            )
        } else {
            format!("`{}` (upstream resolver)", binding.resolver)
        };
        let mut docs = format!(
            "Native binding to {}.\nResolved by {resolution}",
            linked(&binding.target, source_link)
        );
        let matching: Vec<_> = detours
            .iter()
            .filter(|detour| detour.target == binding.target)
            .collect();
        for detour in &matching {
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
        let implementation = native_implementation(
            binding.source.as_ref(),
            &binding.target,
            false,
            &mut watches,
        );
        append_native(
            &mut docs,
            implementation.as_ref(),
            binding.source.as_ref(),
            "Native implementation",
            &binding.target,
        )?;
        write_document(
            &directory,
            &format!("binding-{}", binding.resolver),
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
                "Native implementation",
                &binding.target,
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
    }

    #[test]
    fn code_fences_preserve_embedded_backticks() {
        assert!(block("cpp", "const char* value = \"```\";").starts_with("````cpp"));
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
            "class Native : public Base { public: const FString& Call() const; private: int Value; };").unwrap();
        fs::write(
            root.join("cpp/Engine/Source/Test/Private/Native.cpp"),
            "const FString& Native::Call() const { return Value; }",
        )
        .unwrap();
        fs::write(root.join("src/Chivalry2/Native.rs"), r#"//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Test/Public/Native.h>.
#[runtime_bindings]
pub struct Native;
define_pattern_resolver!(Native__Call, ["40 53 48 83 EC 20 48 8B D9 E8 ?? ?? ?? ?? 48 8B"]);
impl Native {
    /// # Safety
    /// Keep the receiver alive.
    #[bind_resolved_func(Native__Call)]
    pub unsafe fn Call(&self) -> &FString {}
}
"#).unwrap();
        fs::write(
            root.join("src/unchained/patches/Test.rs"),
            "#[bind_func_detour(Native::Call)]\nunsafe fn call_detour() { replacement(); }",
        )
        .unwrap();
        generate(&root, &root.join("out")).unwrap();
        let directory = root.join("out/native-docs");
        let class = fs::read_to_string(directory.join("type-Native.md")).unwrap();
        assert!(class.contains("class Native : public Base"));
        assert!(class.contains("private: int Value;"));
        let binding = fs::read_to_string(directory.join("binding-Native__Call.md")).unwrap();
        assert!(binding.contains("Overwritten by [`call_detour`](file:"));
        assert!(binding.contains(".\nResolved by "));
        assert!(!binding.contains(".\n\nResolved by "));
        assert!(binding.contains("Detours apply only when installed and enabled;"));
        assert!(binding.contains("Keep the receiver alive."));
        assert!(binding.contains("const FString& Native::Call() const { return Value; }"));
        assert!(!binding.contains(CONFIGURATION_NOTE));
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
        let detour = fs::read_to_string(directory.join("detour-call_detour.md")).unwrap();
        assert!(detour.ends_with(NO_CPP_SOURCES));
        fs::write(
            root.join(".cargo/config.local.toml"),
            format!("[env]\nUE_SOURCE_LOCATION = {{ value = \"{UE}\", force = true }}\n"),
        )
        .unwrap();
        generate(&root, &root.join("out")).unwrap();
        let class = fs::read_to_string(directory.join("type-Native.md")).unwrap();
        assert!(class.contains(UE));
        assert!(class.ends_with(CONFIGURATION_NOTE));
        fs::write(root.join("src/unchained/patches/Test.rs"), "").unwrap();
        generate(&root, &root.join("out")).unwrap();
        let binding = fs::read_to_string(directory.join("binding-Native__Call.md")).unwrap();
        assert!(!binding.contains("Overwritten by"));
        assert!(!binding.contains("Detours apply only when"));
        fs::remove_dir_all(root).unwrap();
    }
}
