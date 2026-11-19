#!/usr/bin/env python3
"""Generate a repository-file dependency graph across the chivalry2 boundary."""
import argparse
import collections
import json
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
def inventory():
    default_tmp = "/tmp/opencode" if Path("/tmp/opencode").is_dir() else tempfile.gettempdir()
    cache = Path(os.environ.get("TMPDIR", default_tmp)) / "chivalry2-graph"
    cache.mkdir(parents=True, exist_ok=True)
    source = ROOT / "scripts/chivalry2_graph_inventory.rs"
    manifest = (
        '[package]\nname="chivalry2-graph"\nversion="0.1.0"\nedition="2021"\n'
        '[workspace]\n[dependencies]\nsyn={version="2",features=["full","visit"]}\n'
        'proc-macro2={version="1",features=["span-locations"]}\nquote="1"\nserde_json="1"\n'
        f'[[bin]]\nname="chivalry2-graph"\npath={json.dumps(str(source))}\n'
    )
    (cache / "Cargo.toml").write_text(manifest)
    return json.loads(subprocess.check_output(
        ["cargo", "run", "--quiet", "--manifest-path", str(cache / "Cargo.toml"), "--", str(ROOT)],
        cwd=ROOT, text=True,
    ))


def graph(data):
    modules = {tuple(m["module"]): m for m in data["modules"]}
    definitions = {}
    imports = collections.defaultdict(list)
    globals_ = collections.defaultdict(set)
    bindings = collections.defaultdict(lambda: collections.defaultdict(set))
    for module, item in modules.items():
        for name in item["definitions"]:
            definitions[module + (name,)] = item["file"]
            bindings[module][name].add(module + (name,))
        # macro_use / proc-macro re-exports make these names available unqualified.
        for name in item.get("macros", []):
            globals_[name].add(module + (name,))
            bindings[(module[0],)][name].add(module + (name,))
        if module[:-1] in modules:
            bindings[module[:-1]][module[-1]].add(module)
    for item in data["imports"]:
        imports[tuple(item["module"])].append(item)

    def resolve(module, path):
        path = tuple(path)
        if not path:
            return set()
        head, *tail = path
        if head in ("crate", "$crate"):
            targets = {(module[0],)}
        elif head == "self":
            targets = {module}
        elif head == "super":
            while head == "super":
                module = module[:-1]
                if not tail:
                    return {module}
                head, *tail = tail
            targets = {module}
            tail = [head] + tail
        elif (head,) in modules:
            targets = {(head,)}
        else:
            targets = (bindings[module].get(head, set()) or
                       bindings[(module[0],)].get(head, set()) or globals_.get(head, set()))
        for name in tail:
            next_targets = set()
            for target in targets:
                if target in modules:
                    next_targets.update(bindings[target].get(name, set()))
                elif target in definitions:
                    next_targets.add(target)  # associated method / enum variant
            targets = next_targets
        return targets

    # Resolve aliases and chained re-exports to a fixed point. Expanding globs here
    # does not create edges: only references to imported symbols create edges.
    while True:
        changed = False
        for module, entries in list(imports.items()):
            for imp in entries:
                targets = resolve(module, imp["path"])
                if imp["glob"]:
                    entries_to_add = [
                        (name, values.copy()) for target in targets
                        for name, values in list(bindings[target].items()) if target in modules
                    ]
                else:
                    entries_to_add = [(imp["alias"], targets)]
                for name, values in entries_to_add:
                    previous = len(bindings[module][name])
                    bindings[module][name].update(values)
                    changed |= len(bindings[module][name]) != previous
        if not changed:
            break

    edges = collections.defaultdict(set)
    unresolved = set()

    def add(source, target, line, reference, kind):
        inside = lambda p: p.startswith("src/chivalry2/")
        if source == target or inside(source) == inside(target):
            return
        edges[source, target].add((line, reference, kind))

    for ref in data["references"] + data["imports"]:
        if ref.get("glob"):
            # Resolve individual uses, rather than claiming every glob export is used.
            continue
        targets = resolve(tuple(ref["module"]), ref["path"])
        boundary_candidate = (
            ref["file"].startswith("src/chivalry2/") and
            len(ref["path"]) > 1 and ref["path"][1] != "chivalry2"
        ) or (
            not ref["file"].startswith("src/chivalry2/") and "chivalry2" in ref["path"]
        )
        if boundary_candidate and not targets and len(ref["path"]) > 1 and ref["path"][0] in {
            "crate", "UnchainedPlugin", "unchained_macros", "unchained_macros_impl", "sleuth_macros",
        }:
            unresolved.add((ref["file"], ref["line"], "::".join(ref["path"])))
        for target in targets:
            target_file = modules[target]["file"] if target in modules else definitions.get(target)
            if target_file:
                add(ref["file"], target_file, ref["line"], "::".join(ref["path"]), ref.get("kind", "import"))
    nodes = sorted({p for pair in edges for p in pair})
    return {
        "unresolved_project_paths": [
            {"file": file, "line": line, "path": path}
            for file, line, path in sorted(unresolved)
        ],
        "nodes": [{"id": p, "inside": p.startswith("src/chivalry2/")} for p in nodes],
        "edges": [
            {"source": s, "target": t, "references": [
                {"line": line, "path": path, "kind": kind}
                for line, path, kind in sorted(refs)
            ]}
            for (s, t), refs in sorted(edges.items())
        ],
    }


def dot(data):
    lines = [
        "digraph chivalry2 {", 'graph [rankdir=LR, overlap=false, splines=true];',
        'node [shape=box, style="rounded,filled", fontname="monospace", fontsize=10];',
        'edge [arrowsize=0.6];',
    ]
    for n in data["nodes"]:
        color = "#cce8ff" if n["inside"] else "#ffe2b5"
        lines.append(f'{json.dumps(n["id"])} [fillcolor="{color}", URL={json.dumps("../../" + n["id"])}, target="_blank"];')
    for e in data["edges"]:
        color = "#2374ab" if e["source"].startswith("src/chivalry2/") else "#bd6400"
        lines.append(f'{json.dumps(e["source"])} -> {json.dumps(e["target"])} [color="{color}"];')
    return "\n".join(lines + ["}"]) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", type=Path, help="Reuse an AST inventory (for tests).")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/graphs")
    args = parser.parse_args()
    data = graph(json.loads(args.inventory.read_text()) if args.inventory else inventory())
    args.output.mkdir(parents=True, exist_ok=True)
    prefix = args.output / "chivalry2-boundary"
    prefix.with_suffix(".json").write_text(json.dumps(data, indent=2) + "\n")
    prefix.with_suffix(".dot").write_text(dot(data))
    template = (ROOT / "scripts/chivalry2_graph.html").read_text()
    prefix.with_suffix(".html").write_text(template.replace(
        "/* GRAPH_DATA */", json.dumps(data).replace("<", "\\u003c"),
    ))
    subprocess.run(["dot", "-Tsvg", str(prefix.with_suffix(".dot")),
                    "-o", str(prefix.with_suffix(".svg"))], check=True)
    print(f'{len(data["nodes"])} files, {len(data["edges"])} directed cross-boundary edges')
    print(prefix.with_suffix(".html").relative_to(ROOT) if prefix.is_relative_to(ROOT) else prefix)


if __name__ == "__main__":
    main()
