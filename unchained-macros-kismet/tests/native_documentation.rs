//! Exercise generated documents through both procedural-macro layers and Rustdoc.
#![cfg(unix)]

use std::{fs, path::Path, process::Command};

#[test]
fn generated_native_documents_reach_method_hovers() {
    let root = std::env::temp_dir().join(format!("kismet-native-docs-{}", std::process::id()));
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for directory in [
        "src/Chivalry2",
        "src/unchained/patches",
        ".cargo",
        "cpp/Engine/Source/Test/Public",
        "cpp/Engine/Source/Test/Private",
    ] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    let workspace_manifest = fs::read_to_string(repository.join("Cargo.toml")).unwrap();
    let patches = workspace_manifest
        .split_once("[patch.crates-io]")
        .unwrap()
        .1
        .split("\n[")
        .next()
        .unwrap();
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "native-hover-fixture"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
unchained-macros-core = {{ path = "{}/unchained-macros-core" }}
unchained-macros-kismet = {{ path = "{}/unchained-macros-kismet" }}
[build-dependencies]
unchained-native-docs = {{ path = "{}/build/unchained-native-docs" }}
[patch.crates-io]
{patches}
"#,
            repository.display(),
            repository.display(),
            repository.display()
        ),
    )
    .unwrap();
    fs::copy(repository.join("Cargo.lock"), root.join("Cargo.lock")).unwrap();
    fs::write(
        root.join("build.rs"),
        r#"
fn main() {
    unchained_native_docs::generate(
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()),
    ).unwrap();
}
"#,
    )
    .unwrap();
    fs::write(
        root.join(".cargo/config.local.toml"),
        format!(
            "[env]\nUE_SOURCE_LOCATION = {{ value = \"file://{}/cpp/\", force = true }}\n",
            root.display(),
        ),
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        include_str!("fixtures/native_documentation.rs"),
    )
    .unwrap();
    fs::write(
        root.join("src/Chivalry2/Fixture.rs"),
        include_str!("fixtures/native_documentation.rs"),
    )
    .unwrap();
    fs::write(root.join("cpp/Engine/Source/Test/Public/Fixture.h"),
        "class Reflected { public: static int Call(); static int Call(int Value); static int Typed(int Value); }; class Named { public: static void Scan(); static void Provided(); };").unwrap();
    fs::write(
        root.join("cpp/Engine/Source/Test/Private/Fixture.cpp"),
        r#"
int Reflected::Call() { return SelectedOverload(); }
int Reflected::Call(int Value) { return WrongOverload(); }
int Reflected::Typed(int Value) { return SelectedSignature(); }
int Reflected::Typed() { return WrongSignature(); }
void Named::Scan() { ScanImplementation(); }
void Named::Provided() { ProviderImplementation(); }
"#,
    )
    .unwrap();

    // Build scripts set the docs environment only inside the child Cargo process.
    let output = Command::new(env!("CARGO"))
        .args(["doc", "--offline", "--no-deps"])
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env_remove("UNCHAINED_NATIVE_DOCS_DIR")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let reflected = fs::read_to_string(
        root.join("target/doc/native_hover_fixture/qualified/struct.Reflected.html"),
    )
    .unwrap();
    for expected in [
        "SelectedOverload",
        "SelectedSignature",
        "Reflection native body:",
        "Native implementation:",
        "Fixture.cpp#L",
        "Keep the native receiver alive.",
        "Authored method comment.",
    ] {
        assert!(reflected.contains(expected), "missing {expected}");
    }
    assert_eq!(
        reflected.matches("Keep the native receiver alive.").count(),
        1
    );
    assert_eq!(reflected.matches("Authored method comment.").count(), 1);
    for unexpected in [
        "WrongOverload",
        "WrongSignature",
        "struct.Reflected__Call.html",
        "binding-Call.md",
    ] {
        assert!(!reflected.contains(unexpected), "unexpected {unexpected}");
    }
    let named =
        fs::read_to_string(root.join("target/doc/native_hover_fixture/struct.Named.html")).unwrap();
    for expected in [
        "ScanImplementation",
        "ProviderImplementation",
        "ScanKey",
        "ProviderKey",
    ] {
        assert!(named.contains(expected), "missing {expected}");
    }
    let free =
        fs::read_to_string(root.join("target/doc/native_hover_fixture/fn.free_descriptor.html"))
            .unwrap();
    assert!(free.contains("Direct calls invoke the original implementation"));
    assert!(free.contains("Free-function safety note."));
    fs::remove_dir_all(root).unwrap();
}
