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
unchained-macros-native-docs-generator = {{ path = "{}/unchained-macros-native-docs-generator" }}
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
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    unchained_macros_native_docs_generator::generate(
        &manifest,
        &manifest,
        &manifest.join("generated"),
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
        "class Reflected { public: static int Call(); static int Call(int Value); static int Typed(int Value); static void Observed(); }; class Named { public: static void Scan(); static void Provided(); }; class Explicit { public: static int Direct(int Value); };").unwrap();

    fs::write(
        root.join("cpp/Engine/Source/Test/Private/Fixture.cpp"),
        r#"
int Reflected::Call() { return SelectedOverload(); }
int Reflected::Call(int Value) { return WrongOverload(); }
int Reflected::Typed(int Value) { return SelectedSignature(); }
int Reflected::Typed() { return WrongSignature(); }
void Reflected::Observed() { DummyMustNotBeDiscovered(); }
void Named::Scan() { ScanImplementation(); }
void Named::Provided() { ProviderImplementation(); }
int Explicit::Direct(int Value) { return DescriptorImplementation(); }
int Explicit::Direct() { return WrongDescriptorSignature(); }
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

    let reflected =
        fs::read_to_string(root.join("target/doc/native_hover_fixture/qualified/struct.Reflected.html")).unwrap();

    for expected in [
        "SelectedOverload",
        "SelectedSignature",
        "Reflection native body:",
        "Native implementation:",
        "Fixture.cpp#L",
        "Keep the native receiver alive.",
        "Authored method comment.",
        "Dummy entry safety note.",
        "Entry-only dummy observation marker.",
    ] {
        assert!(reflected.contains(expected), "missing {expected}");
    }

    assert_eq!(reflected.matches("Keep the native receiver alive.").count(), 1);

    assert_eq!(reflected.matches("Authored method comment.").count(), 1);

    for unexpected in [
        "WrongOverload",
        "WrongSignature",
        "struct.Reflected__Call.html",
        "binding-Call.md",
        "DummyMustNotBeDiscovered",
        "Reflected::Observed",
    ] {
        assert!(!reflected.contains(unexpected), "unexpected {unexpected}");
    }

    let generated = root.join("generated/native-docs");

    assert!(generated.join("binding-Reflected__Call.md").exists());

    assert!(!generated.join("binding-Reflected__Observed.md").exists());

    let named = fs::read_to_string(root.join("target/doc/native_hover_fixture/struct.Named.html")).unwrap();

    for expected in ["ScanImplementation", "ProviderImplementation", "ScanKey", "ProviderKey"] {
        assert!(named.contains(expected), "missing {expected}");
    }

    let free = fs::read_to_string(root.join("target/doc/native_hover_fixture/fn.free_descriptor.html")).unwrap();

    assert!(free.contains("Direct calls invoke the original implementation"));

    assert!(free.contains("Free-function safety note."));

    let explicit =
        fs::read_to_string(root.join("target/doc/native_hover_fixture/qualified/struct.Explicit.html")).unwrap();

    for expected in [
        "Explicit::Direct",
        "DescriptorImplementation",
        "Native implementation:",
        "Fixture.cpp#L",
        "Explicit descriptor safety note.",
        "KismetReflectionResolver",
        "FREE_DESCRIPTOR",
        "explicit descriptor",
        "Fixture.rs#L",
    ] {
        assert!(explicit.contains(expected), "missing {expected}");
    }

    for unexpected in [
        "WrongDescriptorSignature",
        "upstream resolver",
        "Reflection native body:",
    ] {
        assert!(!explicit.contains(unexpected), "unexpected {unexpected}");
    }

    assert_eq!(explicit.matches("Explicit descriptor safety note.").count(), 1);

    fs::remove_dir_all(root).unwrap();
}
