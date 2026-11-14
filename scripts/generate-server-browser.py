#!/usr/bin/env python3
# /// script
# dependencies = ["openapi-generator-cli==7.17.0", "PyYAML==6.0.3"]
# ///
"""Generate the Rust SDK from an explicit backend OAS checkout or its pinned snapshot."""

import argparse
import hashlib
import re
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

import openapi_generator_cli
import yaml

ROOT = Path(__file__).resolve().parents[1]
SNAPSHOT = ROOT / "api/server-browser.yaml"
OUTPUT = ROOT / "crates/server-browser-api"
TEMPLATES = OUTPUT / "codegen/templates"


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("--backend-root", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    schema = args.backend_root / "assets/chiv2-server-browser-api.yaml" if args.backend_root else SNAPSHOT
    if args.backend_root and not args.check:
        SNAPSHOT.parent.mkdir(exist_ok=True)
        shutil.copyfile(schema, SNAPSHOT)
    command = ["openapi-generator-cli"]
    version = subprocess.check_output(command + ["version"], text=True).strip()
    if version != "7.17.0":
        raise SystemExit("OpenAPI Generator 7.17.0 is required")
    with tempfile.TemporaryDirectory(prefix="server-browser-rust-") as temporary:
        target = Path(temporary)
        templates = target / "templates"
        shutil.copytree(TEMPLATES, templates)
        jar = Path(openapi_generator_cli.__file__).parent / "openapi-generator.jar"
        with zipfile.ZipFile(jar) as archive:
            (templates / "model-default.mustache").write_bytes(archive.read("rust/model.mustache"))
        document = yaml.safe_load(schema.read_text())
        schemas = document["components"]["schemas"]
        # anyOf order is semantically irrelevant. Try specific shapes first so
        # Serde does not discard diagnostic fields through a matching base type.
        def rust_name(name):
            words = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
            words = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", words)
            return "".join(word.title() for word in words.split("_"))

        def required_fields(definition):
            if "$ref" in definition:
                return required_fields(schemas[definition["$ref"].rsplit("/", 1)[1]])
            fields = set(definition.get("required", []))
            for member in definition.get("allOf", []):
                fields.update(required_fields(member))
            return fields

        for definition in schemas.values():
            if "anyOf" in definition:
                definition["anyOf"].sort(key=lambda item: len(required_fields(item)), reverse=True)
                definition["x-rust-anyof-variants"] = [
                    {"name": rust_name(item["$ref"].rsplit("/", 1)[1])}
                    for item in definition["anyOf"]
                ]
        normalized = target / "schema.yaml"
        normalized.write_text(yaml.safe_dump(document, sort_keys=False))
        subprocess.run(command + [
            "generate", "-g", "rust", "-i", str(normalized), "-o", str(target), "-t", str(templates),
            "--additional-properties=packageName=server-browser-api,packageVersion=1.0.0,supportAsync=false,avoidBoxedModels=true",
            "--global-property=apiDocs=false,modelDocs=false,apiTests=false,modelTests=false",
        ], check=True)
        # Own only Rust source and its manifest, not generator README/CI scaffolding.
        files = [Path("Cargo.toml"), *(path.relative_to(target) for path in (target / "src").rglob("*.rs"))]
        for path in files:
            text = (target / path).read_text()
            (target / path).write_text("\n".join(line.rstrip() for line in text.splitlines()).strip() + "\n")
        expected = set(files)
        actual = {path.relative_to(OUTPUT) for path in OUTPUT.rglob("*.rs")} | ({Path("Cargo.toml")} if (OUTPUT / "Cargo.toml").exists() else set())
        if args.check:
            if expected != actual or any(not (OUTPUT / path).exists() or (OUTPUT / path).read_bytes() != (target / path).read_bytes() for path in files):
                raise SystemExit("Generated Rust SDK is stale")
            if args.backend_root and schema.read_bytes() != SNAPSHOT.read_bytes():
                raise SystemExit("Pinned OAS differs from backend")
        else:
            for obsolete in actual - expected:
                (OUTPUT / obsolete).unlink()
            for path in files:
                (OUTPUT / path).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(target / path, OUTPUT / path)
    print(f"Rust SDK current; OAS SHA-256 {hashlib.sha256(schema.read_bytes()).hexdigest()}")


if __name__ == "__main__":
    main()
