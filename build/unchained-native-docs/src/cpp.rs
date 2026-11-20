use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser as CppParser, Tree};

#[derive(Debug, Clone)]
pub struct Implementation {
    pub path: PathBuf,
    pub line: usize,
    pub code: String,
}

struct SourceFile {
    bytes: Vec<u8>,
    declarations: Option<Declarations>,
}

struct Definition {
    name: String,
    line: usize,
    range: std::ops::Range<usize>,
    overload: Option<Vec<Vec<String>>>,
}

impl Definition {
    fn matches(&self, target: &str) -> bool {
        self.name == target
            || self
                .name
                .strip_suffix(target)
                .is_some_and(|prefix| prefix.ends_with("::"))
    }

    fn implementation(&self, path: &Path, source: &str) -> Implementation {
        Implementation {
            path: path.to_owned(),
            line: self.line,
            code: source[self.range.clone()].to_owned(),
        }
    }
}

#[derive(Default)]
struct Declarations {
    classes: Vec<Definition>,
    functions: Vec<Definition>,
}

impl SourceFile {
    fn parse<'a>(&'a mut self, path: &Path) -> Result<(&'a str, &'a Declarations)> {
        let source = std::str::from_utf8(&self.bytes)
            .with_context(|| format!("Native source is not UTF-8: {}", path.display()))?;
        if self.declarations.is_none() {
            let tree = parse_source(source)?;
            let mut declarations = Declarations::default();
            class_definitions(tree.root_node(), source, &mut declarations.classes);
            definitions(tree.root_node(), source, None, &mut declarations.functions);
            self.declarations = Some(declarations);
        }
        Ok((source, self.declarations.as_ref().unwrap()))
    }
}

/// Build-local source index. Planned module lookups share one read/parse pass.
/// Create a fresh index after source changes; nothing persists across builds.
#[derive(Default)]
pub struct SourceIndex {
    files: BTreeMap<PathBuf, SourceFile>,
    modules: BTreeMap<PathBuf, ModuleIndex>,
    requests: BTreeMap<PathBuf, BTreeSet<String>>,
}

struct ModuleIndex {
    files: BTreeSet<PathBuf>,
    functions: BTreeMap<String, std::result::Result<Vec<IndexedFunction>, String>>,
}

struct IndexedFunction {
    definition: Definition,
    implementation: Implementation,
}

impl SourceIndex {
    pub fn new() -> Self {
        Self::default()
    }

    fn load(&mut self, path: &Path) -> Result<()> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.files.entry(path.to_owned())
        {
            entry.insert(SourceFile {
                bytes: fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?,
                declarations: None,
            });
        }
        Ok(())
    }

    /// Register all planned method lookups before generation to batch file reads.
    pub fn plan_implementation(&mut self, root: &Path, header: &Path, target: &str) -> Result<()> {
        let (root, header) = native_header(root, header)?;
        self.requests
            .entry(module_directory(&header, &root))
            .or_default()
            .insert(method_name(target));
        Ok(())
    }

    fn index_module(&mut self, module: &Path, method: &str) -> Result<()> {
        self.requests
            .entry(module.to_owned())
            .or_default()
            .insert(method.to_owned());
        if let std::collections::btree_map::Entry::Vacant(entry) =
            self.modules.entry(module.to_owned())
        {
            let mut files = BTreeSet::new();
            cpp_files(module, &mut files)?;
            entry.insert(ModuleIndex {
                files,
                functions: BTreeMap::new(),
            });
        }
        let indexed = &self.modules[module];
        let mut pending: BTreeMap<_, std::result::Result<Vec<IndexedFunction>, String>> = self
            .requests[module]
            .iter()
            .filter(|method| !indexed.functions.contains_key(*method))
            .map(|method| (method.clone(), Ok(Vec::new())))
            .collect();
        if pending.is_empty() {
            return Ok(());
        }
        let wanted: BTreeSet<_> = pending.keys().cloned().collect();
        for path in &indexed.files {
            let bytes =
                fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?;
            if !pending
                .keys()
                .any(|method| memchr::memmem::find(&bytes, method.as_bytes()).is_some())
            {
                continue;
            }
            let parsed = (|| -> Result<Vec<Definition>> {
                let source = std::str::from_utf8(&bytes)
                    .with_context(|| format!("Native source is not UTF-8: {}", path.display()))?;
                let tree = parse_source(source)?;
                let mut found = Vec::new();
                definitions(tree.root_node(), source, Some(&wanted), &mut found);
                Ok(found)
            })();
            match parsed {
                Ok(found) => {
                    let source = std::str::from_utf8(&bytes).unwrap();
                    for definition in found {
                        let method = definition.name.rsplit("::").next().unwrap();
                        if let Some(Ok(functions)) = pending.get_mut(method) {
                            let implementation = definition.implementation(path, source);
                            functions.push(IndexedFunction {
                                definition,
                                implementation,
                            });
                        }
                    }
                }
                Err(error) => {
                    for (method, functions) in &mut pending {
                        if memchr::memmem::find(&bytes, method.as_bytes()).is_some() {
                            *functions = Err(error.to_string());
                        }
                    }
                }
            }
        }
        self.modules
            .get_mut(module)
            .unwrap()
            .functions
            .extend(pending);
        Ok(())
    }
}

fn method_name(target: &str) -> String {
    let target = target.split('(').next().unwrap().trim();
    if let Some(owner) = target.strip_suffix("::Destructor") {
        format!("~{}", owner.rsplit("::").next().unwrap())
    } else {
        target.rsplit("::").next().unwrap().to_owned()
    }
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

pub(crate) fn module_directory(header: &Path, root: &Path) -> PathBuf {
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

fn function_declarator(node: Node<'_>) -> Option<Node<'_>> {
    let mut declarator = node.child_by_field_name("declarator")?;
    while declarator.kind() != "function_declarator" {
        declarator = declarator.child_by_field_name("declarator").or_else(|| {
            let mut cursor = declarator.walk();
            let found = declarator
                .named_children(&mut cursor)
                .find(|child| child.kind().ends_with("_declarator"));
            found
        })?;
    }
    Some(declarator)
}

fn declarator_name(node: Node<'_>, source: &str) -> Option<String> {
    let name = function_declarator(node)?.child_by_field_name("declarator")?;
    Some(
        source[name.byte_range()]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect(),
    )
}

// Compare source-level types, not Rust ABI arguments (which may include sret).
fn overload_key(node: Node<'_>, source: &str) -> Option<Vec<Vec<String>>> {
    fn tokens(node: Node<'_>, source: &str, skip: Option<usize>, out: &mut Vec<String>) {
        if Some(node.start_byte()) == skip || node.kind() == "comment" {
            return;
        }
        if node.child_count() == 0 {
            out.push(source[node.byte_range()].to_owned());
        } else {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                tokens(child, source, skip, out);
            }
        }
    }

    let declarator = function_declarator(node)?;
    let parameters = declarator.child_by_field_name("parameters")?;
    let mut key = Vec::new();
    let mut cursor = parameters.walk();
    for parameter in parameters.children(&mut cursor) {
        if matches!(parameter.kind(), "(" | ")" | "," | "comment") {
            continue;
        }
        if parameter.kind() == "..." {
            key.push(vec!["...".to_owned()]);
            continue;
        }
        let mut name = parameter.child_by_field_name("declarator");
        while let Some(node) = name {
            // Complex callable declarators need a richer type model.
            if node.kind() == "function_declarator" {
                return None;
            }
            if node.kind() == "identifier" {
                break;
            }
            name = node.child_by_field_name("declarator").or_else(|| {
                let mut cursor = node.walk();
                let found = node.named_children(&mut cursor).find(|child| {
                    child.kind() == "identifier" || child.kind().ends_with("_declarator")
                });
                found
            });
        }
        let mut parameter_key = Vec::new();
        let mut cursor = parameter.walk();
        for child in parameter.children(&mut cursor) {
            if child.kind() == "=" {
                break;
            }
            tokens(
                child,
                source,
                name.map(|node| node.start_byte()),
                &mut parameter_key,
            );
        }
        key.push(parameter_key);
    }
    let mut qualifiers = Vec::new();
    let mut cursor = declarator.walk();
    for child in declarator.named_children(&mut cursor) {
        if matches!(child.kind(), "type_qualifier" | "ref_qualifier") {
            tokens(child, source, None, &mut qualifiers);
        }
    }
    key.push(qualifiers);
    Some(key)
}

fn qualified_name(node: Node<'_>, source: &str) -> Option<String> {
    let name = declarator_name(node, source)?;
    scoped_name(node, name, source)
}

fn scoped_name(node: Node<'_>, name: String, source: &str) -> Option<String> {
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

fn parser_input(source: &str) -> Vec<u8> {
    let mut input = source.as_bytes().to_vec();
    // Unreal's export annotations are not C++ keywords. Keep original byte offsets.
    for (offset, _) in source.match_indices("_API") {
        let end = offset + 4;
        let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
        if source[end..].starts_with(identifier) {
            continue;
        }
        let start = source[..offset]
            .rfind(|c: char| !identifier(c))
            .map_or(0, |index| {
                index + source[index..].chars().next().unwrap().len_utf8()
            });
        let prefix = source[..start].trim_end();
        let keyword = prefix.rsplit(|c: char| !identifier(c)).next().unwrap_or("");
        if matches!(keyword, "class" | "struct") {
            input[start..end].fill(b' ');
        }
    }
    input
}

fn parse_source(source: &str) -> Result<Tree> {
    let mut parser = CppParser::new();
    parser.set_language(&tree_sitter_cpp::LANGUAGE.into())?;
    parser
        .parse(parser_input(source), None)
        .context("C++ parsing was cancelled")
}

fn class_definitions(node: Node<'_>, source: &str, found: &mut Vec<Definition>) {
    if matches!(
        node.kind(),
        "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier"
    ) {
        if let (Some(name), Some(body)) = (
            node.child_by_field_name("name"),
            node.child_by_field_name("body"),
        ) {
            let closed = body
                .child(body.child_count().saturating_sub(1))
                .is_some_and(|end| end.kind() == "}" && !end.is_missing());
            if closed {
                if let Some(name) = scoped_name(node, source[name.byte_range()].to_owned(), source)
                {
                    let declaration = node
                        .parent()
                        .filter(|parent| parent.kind() == "template_declaration")
                        .unwrap_or(node);
                    let mut end = declaration.end_byte();
                    let suffix = &source[end..];
                    let whitespace = suffix.len() - suffix.trim_start().len();
                    if suffix[whitespace..].starts_with(';') {
                        end += whitespace + 1;
                    }
                    found.push(Definition {
                        name,
                        line: declaration.start_position().row + 1,
                        range: declaration.start_byte()..end,
                        overload: None,
                    });
                }
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        class_definitions(child, source, found);
    }
}

fn native_header(root: &Path, header: &Path) -> Result<(PathBuf, PathBuf)> {
    let root = root.canonicalize().context("Cannot open source root")?;
    let header = root
        .join(header)
        .canonicalize()
        .context("Cannot open native header")?;
    anyhow::ensure!(
        header.starts_with(&root),
        "Native header is outside the source root"
    );
    Ok((root, header))
}

pub fn find_class(root: &Path, header: &Path, target: &str) -> Result<Option<Implementation>> {
    SourceIndex::new().find_class(root, header, target)
}

impl SourceIndex {
    pub fn find_class(
        &mut self,
        root: &Path,
        header: &Path,
        target: &str,
    ) -> Result<Option<Implementation>> {
        let (_, header) = native_header(root, header)?;
        self.load(&header)?;
        let (source, declarations) = self.files.get_mut(&header).unwrap().parse(&header)?;
        let mut found = declarations
            .classes
            .iter()
            .filter(|definition| definition.matches(target));
        let first = found.next();
        Ok(if found.next().is_none() {
            first.map(|definition| definition.implementation(&header, source))
        } else {
            None
        })
    }
}

fn definitions(
    node: Node<'_>,
    source: &str,
    wanted: Option<&BTreeSet<String>>,
    found: &mut Vec<Definition>,
) {
    if node.kind() == "function_definition" {
        if !node.has_error() && node.child_by_field_name("body").is_some() {
            if let Some(name) = qualified_name(node, source) {
                if wanted
                    .is_some_and(|methods| !methods.contains(name.rsplit("::").next().unwrap()))
                {
                    return;
                }
                let declaration = node
                    .parent()
                    .filter(|parent| parent.kind() == "template_declaration")
                    .unwrap_or(node);
                found.push(Definition {
                    name,
                    line: declaration.start_position().row + 1,
                    range: declaration.byte_range(),
                    overload: overload_key(node, source),
                });
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        definitions(child, source, wanted, found);
    }
}

pub fn find_implementation(
    root: &Path,
    header: &Path,
    target: &str,
) -> Result<Option<Implementation>> {
    SourceIndex::new().find_implementation(root, header, target)
}

impl SourceIndex {
    pub fn find_implementation(
        &mut self,
        root: &Path,
        header: &Path,
        target: &str,
    ) -> Result<Option<Implementation>> {
        let (target, selector) = target
            .split_once('(')
            .map_or((target, None), |(name, parameters)| {
                (name.trim(), Some(parameters))
            });
        anyhow::ensure!(
            target
                .rsplit_once("::")
                .is_some_and(|(owner, method)| !owner.is_empty() && !method.is_empty()),
            "Expected a Class::Method target"
        );
        let (root, header) = native_header(root, header)?;
        let module = module_directory(&header, &root);
        let overload = if let Some(selector) = selector {
            let source = format!("void selected({selector} {{}}");
            let tree = parse_source(&source)?;
            anyhow::ensure!(
                !tree.root_node().has_error(),
                "Invalid C++ overload selector"
            );
            let function = tree
                .root_node()
                .named_child(0)
                .context("Missing overload selector")?;
            Some(overload_key(function, &source).context("Unsupported C++ overload selector")?)
        } else {
            None
        };
        let target = if let Some(owner) = target.strip_suffix("::Destructor") {
            format!("{owner}::~{}", owner.rsplit("::").next().unwrap())
        } else {
            target.to_owned()
        };
        let method = target.rsplit("::").next().unwrap();
        let mut implementations = Vec::new();
        self.load(&header)?;
        let file = self.files.get_mut(&header).unwrap();
        if memchr::memmem::find(&file.bytes, method.as_bytes()).is_some() {
            let (source, declarations) = file.parse(&header)?;
            implementations.extend(
                declarations
                    .functions
                    .iter()
                    .filter(|definition| {
                        definition.matches(&target)
                            && overload
                                .as_ref()
                                .is_none_or(|key| definition.overload.as_ref() == Some(key))
                    })
                    .map(|definition| definition.implementation(&header, source)),
            );
        }
        self.index_module(&module, method)?;
        let functions = self.modules[&module].functions[method]
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        implementations.extend(
            functions
                .iter()
                .filter(|function| {
                    function.implementation.path != header
                        && function.definition.matches(&target)
                        && overload
                            .as_ref()
                            .is_none_or(|key| function.definition.overload.as_ref() == Some(key))
                })
                .map(|function| function.implementation.clone()),
        );
        // Never choose an overload or preprocessor branch by directory order.
        Ok(if implementations.len() == 1 {
            implementations.pop()
        } else {
            None
        })
    }
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
    fn source_index_reuses_files_and_trees_within_one_generation() {
        let fixture = Fixture::new();
        let header = Path::new("Source/Test/Public/Native.h");
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { int Call(); int Other(); };",
        );
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "int Native::Call() { return 1; }\nint Native::Other() { return 2; }",
        );
        fixture.write("Source/Test/Private/Unrelated.cpp", "void Unrelated() {}");
        let mut index = SourceIndex::new();
        index
            .plan_implementation(&fixture.0, header, "Native::Call()")
            .unwrap();
        index
            .plan_implementation(&fixture.0, header, "Native::Other")
            .unwrap();
        assert!(index
            .find_class(&fixture.0, header, "Native")
            .unwrap()
            .is_some());
        assert!(index
            .find_implementation(&fixture.0, header, "Native::Call")
            .unwrap()
            .unwrap()
            .code
            .contains("return 1;"));
        assert!(index
            .find_implementation(&fixture.0, header, "Native::Other")
            .unwrap()
            .unwrap()
            .code
            .contains("return 2;"));
        assert_eq!(index.modules.len(), 1);
        assert_eq!(index.files.len(), 1);
        let module = index.modules.values().next().unwrap();
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions["Call"].as_ref().unwrap().len(), 1);
        assert_eq!(module.functions["Other"].as_ref().unwrap().len(), 1);
        assert_eq!(
            index
                .files
                .values()
                .filter(|file| file.declarations.is_some())
                .count(),
            1
        );

        fixture.write(
            "Source/Test/Private/Native.cpp",
            "int Native::Call() { return 3; }",
        );
        assert!(index
            .find_implementation(&fixture.0, header, "Native::Call")
            .unwrap()
            .unwrap()
            .code
            .contains("return 1;"));
        let mut fresh = SourceIndex::new();
        assert!(fresh
            .find_implementation(&fixture.0, header, "Native::Call")
            .unwrap()
            .unwrap()
            .code
            .contains("return 3;"));
        fixture.write(
            "Source/Test/Private/Duplicate.cpp",
            "int Native::Call() { return 4; }",
        );
        assert!(SourceIndex::new()
            .find_implementation(&fixture.0, header, "Native::Call")
            .unwrap()
            .is_none());
    }

    #[test]
    fn unplanned_methods_extend_the_index_without_losing_matches() {
        let fixture = Fixture::new();
        let header = Path::new("Source/Test/Public/Native.h");
        fixture.write("Source/Test/Public/Native.h", "struct Native {};");
        fixture.write("Source/Test/Private/First.cpp", "void Native::Call() {}");
        fixture.write("Source/Test/Private/Second.cpp", "void Native::Other() {}");
        let mut index = SourceIndex::new();
        assert!(index
            .find_implementation(&fixture.0, header, "Native::Call")
            .unwrap()
            .is_some());
        assert!(index
            .find_implementation(&fixture.0, header, "Native::Other")
            .unwrap()
            .is_some());
        assert_eq!(index.modules.len(), 1);
    }

    #[test]
    fn planned_method_errors_do_not_hide_other_methods() {
        let fixture = Fixture::new();
        let header = Path::new("Source/Test/Public/Native.h");
        fixture.write("Source/Test/Public/Native.h", "struct Native {};");
        fixture.write("Source/Test/Private/Native.cpp", "void Native::Call() {}");
        fs::write(
            fixture.0.join("Source/Test/Private/Other.cpp"),
            b"\xff Native::Other",
        )
        .unwrap();
        let mut index = SourceIndex::new();
        index
            .plan_implementation(&fixture.0, header, "Native::Call")
            .unwrap();
        index
            .plan_implementation(&fixture.0, header, "Native::Other")
            .unwrap();
        assert!(index
            .find_implementation(&fixture.0, header, "Native::Call")
            .unwrap()
            .is_some());
        assert!(index
            .find_implementation(&fixture.0, header, "Native::Other")
            .is_err());
    }

    #[test]
    fn unrelated_non_utf8_sources_are_not_parsed() {
        let fixture = Fixture::new();
        fixture.write("Source/Test/Public/Native.h", "struct Native {};");
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "int Native::Call() { return 1; }",
        );
        fs::write(
            fixture.0.join("Source/Test/Private/Unrelated.cpp"),
            [0xff, 0xfe],
        )
        .unwrap();
        assert!(fixture.find("Native::Call").is_some());
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
    fn handles_const_reference_return_declarators() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "struct Native { const FString& Call() const; };",
        );
        fixture.write("Source/Test/Private/Native.cpp",
            "const FString& Native::Call() const\n{\n    Rebuild();\n    return TextData->GetDisplayString();\n}");
        assert!(fixture
            .find("Native::Call")
            .unwrap()
            .code
            .contains("return TextData->GetDisplayString();"));
    }

    #[test]
    fn selects_callback_overload_in_cpp_instead_of_inline_delegate_wrapper() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "class TEST_API Native { public: static void Call(const String& Address, float Timeout, Delegate Result) { Forward(Result); } };",
        );
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "void Native::Call(const String& Address, float Timeout, Callback Result) { Send(Result); }",
        );
        assert!(fixture.find("Native::Call").is_none());
        let callback = fixture
            .find("Native::Call(const String&, float, Callback)")
            .unwrap();
        assert!(callback.path.ends_with("Native.cpp"));
        assert!(callback.code.contains("Send(Result);"));
        let delegate = fixture
            .find("Native::Call(const String&, float, Delegate)")
            .unwrap();
        assert!(delegate.path.ends_with("Native.h"));
        assert!(delegate.code.contains("Forward(Result);"));
    }

    #[test]
    fn selects_overloads_by_cpp_parameter_types() {
        let fixture = Fixture::new();
        fixture.write("Source/Test/Public/Native.h", "struct Native {};");
        fixture.write(
            "Source/Test/Private/Native.cpp",
            r#"
Text Native::Call(const String& Value) { return copy(Value); }
Text Native::Call(String&& Value) { return move(Value); }
Text Native::Call(Text Value) { return Value; }
Text Native::Call(const String* Value, int Count = 0) { return pointer(Value); }
Text Native::Call(int Count, ...) { return variadic(Count); }
"#,
        );
        for (selector, body) in [
            ("const String&", "copy(Value)"),
            ("String &&", "move(Value)"),
            ("Text", "return Value;"),
            ("const String *, int", "pointer(Value)"),
            ("int, ...", "variadic(Count)"),
        ] {
            let found = fixture
                .find(&format!("Native::Call({selector})"))
                .unwrap_or_else(|| panic!("No match for {selector}"));
            assert!(found.code.contains(body), "{selector}: {}", found.code);
        }
        assert!(fixture.find("Native::Call").is_none());
        assert!(fixture.find("Native::Call(String&)").is_none());
        assert!(fixture.find("Native::Call()").is_none());
        assert!(fixture.find("Native::Call(int)").is_none());
    }

    #[test]
    fn selects_cv_ref_overloads_but_rejects_duplicate_definitions() {
        let fixture = Fixture::new();
        fixture.write("Source/Test/Public/Native.h", "struct Native {};");
        fixture.write(
            "Source/Test/Private/Native.cpp",
            "int Native::Call() { return 1; }\nint Native::Call() const { return 2; }\nint Native::Call() && { return 3; }",
        );
        assert!(fixture
            .find("Native::Call()")
            .unwrap()
            .code
            .contains("return 1;"));
        assert!(fixture
            .find("Native::Call() const")
            .unwrap()
            .code
            .contains("return 2;"));
        assert!(fixture
            .find("Native::Call() &&")
            .unwrap()
            .code
            .contains("return 3;"));
        fixture.write(
            "Source/Test/Private/Other.cpp",
            "int Native::Call() const { return 4; }",
        );
        assert!(fixture.find("Native::Call() const").is_none());
        assert!(find_implementation(
            &fixture.0,
            Path::new("Source/Test/Public/Native.h"),
            "Native::Call(broken("
        )
        .is_err());
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

    #[test]
    fn extracts_complete_exported_class_with_bases_and_members() {
        let fixture = Fixture::new();
        let class = r#"class TEST_API Native final : public Base, public Interface {
    GENERATED_BODY()
public:
    UFUNCTION(BlueprintCallable)
    void Call();
    int Inline() const { return 42; }
private:
    UPROPERTY()
    int Value;
};"#;
        fixture.write(
            "Source/Test/Public/Native.h",
            &format!("class Native;\nUCLASS(Blueprintable)\n{class}\nstruct Other {{}};"),
        );
        let found = find_class(
            &fixture.0,
            Path::new("Source/Test/Public/Native.h"),
            "Native",
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.code, class);
        assert_eq!(found.line, 3);
    }

    #[test]
    fn class_lookup_supports_qualified_names_and_templates() {
        let fixture = Fixture::new();
        fixture.write(
            "Source/Test/Public/Native.h",
            "namespace Game {\ntemplate<class T> struct Native { T Value; };\n}",
        );
        let found = find_class(
            &fixture.0,
            Path::new("Source/Test/Public/Native.h"),
            "Game::Native",
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.code, "template<class T> struct Native { T Value; };");
    }

    #[test]
    fn type_lookup_supports_complete_enums_and_unions() {
        let fixture = Fixture::new();
        for source in [
            "enum class Native : unsigned char { First, Second };",
            "union Native { int Value; void* Pointer; };",
        ] {
            fixture.write("Source/Test/Public/Native.h", source);
            let found = find_class(
                &fixture.0,
                Path::new("Source/Test/Public/Native.h"),
                "Native",
            )
            .unwrap()
            .unwrap();
            assert_eq!(found.code, source);
        }
    }

    #[test]
    fn class_lookup_refuses_forward_incomplete_and_ambiguous_definitions() {
        let fixture = Fixture::new();
        for source in [
            "class Native;",
            "class Native { int Value;",
            "namespace A { class Native {}; } namespace B { class Native {}; }",
        ] {
            fixture.write("Source/Test/Public/Native.h", source);
            assert!(
                find_class(
                    &fixture.0,
                    Path::new("Source/Test/Public/Native.h"),
                    "Native"
                )
                .unwrap()
                .is_none(),
                "{source}"
            );
        }
    }
}
