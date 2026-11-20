use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use tree_sitter::{Node, Parser as CppParser};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    root: PathBuf,
    #[arg(long)]
    header: PathBuf,
    #[arg(long)]
    target: String,
}

#[derive(Debug)]
struct Implementation {
    path: PathBuf,
    line: usize,
    code: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let result = find_implementation(&args.root, &args.header, &args.target)?;
    let output = result.map(|implementation| {
        serde_json::json!({
            "path": implementation.path,
            "line": implementation.line,
            "code": implementation.code,
        })
    });
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

fn cpp_files(directory: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        // Do not follow directory symlinks outside the selected checkout.
        if kind.is_dir() {
            cpp_files(&entry.path(), files)?;
        } else if kind.is_file()
            && matches!(
                entry.path().extension().and_then(|s| s.to_str()),
                Some("cpp" | "cc" | "cxx")
            )
        {
            files.insert(entry.path());
        }
    }
    Ok(())
}

fn module_directory(header: &Path, root: &Path) -> PathBuf {
    for directory in header.ancestors().skip(1) {
        if matches!(
            directory.file_name().and_then(|s| s.to_str()),
            Some("Public" | "Private" | "Classes")
        ) {
            return directory
                .parent()
                .filter(|parent| parent.starts_with(root))
                .unwrap_or(root)
                .to_path_buf();
        }
        if directory == root {
            break;
        }
    }
    header.parent().unwrap_or(root).to_path_buf()
}

fn declarator_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut declarator = node.child_by_field_name("declarator")?;
    while declarator.kind() != "function_declarator" {
        declarator = declarator.child_by_field_name("declarator")?;
    }
    let name = declarator.child_by_field_name("declarator")?;
    Some(
        source[name.byte_range()]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect(),
    )
}

fn qualified_name(node: Node<'_>, source: &str) -> Option<String> {
    let name = declarator_name(node, source)?;
    let mut scopes = Vec::new();
    let mut parent = node.parent();
    while let Some(node) = parent {
        if matches!(
            node.kind(),
            "class_specifier" | "struct_specifier" | "namespace_definition"
        ) {
            if let Some(name) = node.child_by_field_name("name") {
                scopes.push(source[name.byte_range()].to_owned());
            } else {
                // Anonymous scopes cannot be identified by a Rust class name.
                return None;
            }
        }
        parent = node.parent();
    }
    scopes.reverse();
    scopes.push(name);
    Some(scopes.join("::"))
}

fn definitions(node: Node<'_>, source: &str, target: &str, found: &mut Vec<(usize, String)>) {
    if node.kind() == "function_definition" {
        if !node.has_error() && node.child_by_field_name("body").is_some() {
            if let Some(name) = qualified_name(node, source) {
                if name == target || name.ends_with(&format!("::{target}")) {
                    let declaration = node
                        .parent()
                        .filter(|parent| parent.kind() == "template_declaration")
                        .unwrap_or(node);
                    found.push((
                        declaration.start_position().row + 1,
                        source[declaration.byte_range()].to_owned(),
                    ));
                }
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        definitions(child, source, target, found);
    }
}

fn find_implementation(root: &Path, header: &Path, target: &str) -> Result<Option<Implementation>> {
    anyhow::ensure!(
        target
            .rsplit_once("::")
            .is_some_and(|(owner, method)| !owner.is_empty() && !method.is_empty()),
        "Expected a Class::Method target"
    );
    let root = root.canonicalize().context("Cannot open source root")?;
    let header = root
        .join(header)
        .canonicalize()
        .context("Cannot open native header")?;
    anyhow::ensure!(
        header.starts_with(&root),
        "Native header is outside the source root"
    );
    let mut files = BTreeSet::from([header.clone()]);
    cpp_files(&module_directory(&header, &root), &mut files)?;
    let mut parser = CppParser::new();
    parser.set_language(&tree_sitter_cpp::LANGUAGE.into())?;
    let target = if let Some(owner) = target.strip_suffix("::Destructor") {
        format!("{owner}::~{}", owner.rsplit("::").next().unwrap())
    } else {
        target.to_owned()
    };
    let method = target.rsplit("::").next().unwrap();
    let mut implementations = Vec::new();
    for path in files {
        let bytes = fs::read(&path).with_context(|| format!("Cannot read {}", path.display()))?;
        if !bytes
            .windows(method.len())
            .any(|window| window == method.as_bytes())
        {
            continue;
        }
        let source = String::from_utf8(bytes).with_context(|| {
            format!(
                "Native implementation source is not UTF-8: {}",
                path.display()
            )
        })?;
        let tree = parser
            .parse(&source, None)
            .context("C++ parsing was cancelled")?;
        let mut found = Vec::new();
        definitions(tree.root_node(), &source, &target, &mut found);
        implementations.extend(found.into_iter().map(|(line, code)| Implementation {
            path: path.clone(),
            line,
            code,
        }));
        // Never choose an overload or preprocessor branch by directory order.
        if implementations.len() > 1 {
            return Ok(None);
        }
    }
    Ok(implementations.pop())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "native-cpp-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir_all(root.join("Source/Test/Public")).unwrap();
            fs::create_dir_all(root.join("Source/Test/Private")).unwrap();
            Self(root)
        }

        fn write(&self, path: &str, code: &str) {
            fs::write(self.0.join(path), code).unwrap();
        }

        fn find(&self, target: &str) -> Option<Implementation> {
            find_implementation(&self.0, Path::new("Source/Test/Public/Native.h"), target).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only this test's uniquely created fixture directory.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn extracts_complete_body_without_matching_calls_comments_or_strings() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { int Call(); };",
        );
        fixture.write(
            "Source/Test/Private/DifferentFilename.cpp",
            r#"
// int Native::Call() { not_an_implementation(); }
const char* example = "Native::Call() { }";
int Other::Call() { return 0; }
int Native::Call() {
    auto inner = [] { return "}"; };
    if (ready()) { use(inner()); }
    return 42;
}
"#,
        );
        let found = fixture.find("Native::Call").unwrap();
        assert_eq!(found.line, 5);
        assert!(found.code.starts_with("int Native::Call() {"));
        assert!(found.code.ends_with("return 42;\n}"));
        assert!(found.path.ends_with("DifferentFilename.cpp"));
    }

    #[test]
    fn handles_inline_namespaces_and_pointer_returns() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            r#"
namespace Game {
struct Native { int* Call() const { return nullptr; } };
}
"#,
        );
        let found = fixture.find("Native::Call").unwrap();
        assert_eq!(found.code, "int* Call() const { return nullptr; }");
    }

    #[test]
    fn refuses_overloads_and_duplicate_implementations() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { void Call(); };",
        );
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "void Native::Call() {}\nvoid Native::Call(int value) {}",
        );
        assert!(fixture.find("Native::Call").is_none());
    }

    #[test]
    fn does_not_treat_declarations_as_implementations() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { void Call(); };",
        );
        assert!(fixture.find("Native::Call").is_none());
    }

    #[test]
    fn preserves_template_declarations() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { template<class T> T Call(T value); };",
        );
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "template<class T>\nT Native::Call(T value) { return value; }",
        );
        assert!(fixture
            .find("Native::Call")
            .unwrap()
            .code
            .starts_with("template<class T>"));
    }

    #[test]
    fn refuses_alternative_preprocessor_definitions_and_malformed_bodies() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { void Call(); };",
        );
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "#if STEAM\nvoid Native::Call() {}\n#else\nvoid Native::Call() {}\n#endif",
        );
        assert!(fixture.find("Native::Call").is_none());
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "void Native::Call() { if ( }",
        );
        assert!(fixture.find("Native::Call").is_none());
    }

    #[test]
    fn supports_destructor_binding_names() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { ~Native() {} };",
        );
        assert_eq!(
            fixture.find("Native::Destructor").unwrap().code,
            "~Native() {}"
        );
    }

    #[test]
    fn refuses_headers_outside_the_root() {
        let fixture = Fixture::new();
        assert!(
            find_implementation(&fixture.0, Path::new("../../../Cargo.toml"), "Native::Call")
                .is_err()
        );
    }

    #[test]
    fn does_not_search_outside_a_public_directory_root() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { void Call(); };",
        );
        fixture.write("Source/Test/Private/Native.cpp", "void Native::Call() {}");
        assert!(find_implementation(
            &fixture.0.join("Source/Test/Public"),
            Path::new("Native.h"),
            "Native::Call",
        )
        .unwrap()
        .is_none());
    }
}
