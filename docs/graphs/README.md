# chivalry2 boundary graph

Open **[chivalry2-boundary.html](chivalry2-boundary.html)** in a browser.
It is self-contained and works offline; GitHub does not render the HTML page.
Search by filename, filter direction, or click a file to isolate its neighbors.
Click an arrow or connection button for its source lines and reference paths.

- [SVG](chivalry2-boundary.svg): static overview; zoom to read labels.
- [DOT](chivalry2-boundary.dot): Graphviz input.
- [JSON](chivalry2-boundary.json): nodes, directed edges, and source evidence.

Every node is a repository file. An arrow means **referencing file → referenced
file**. Blue arrows leave `src/chivalry2/`; orange arrows enter it. Same-side
edges and files without a resolved cross-boundary connection are omitted.

## Scope

The generator parses Rust syntax rather than grepping source text:

- Follows module files and inline modules, including tests and all cfg branches.
- Resolves explicit paths, aliases, re-exports, associated paths, and used glob
  imports to the defining file.
- Includes native binding attributes, macro invocations, and paths written in
  macro token streams. Repository macro crates are file nodes too.
- Includes the plugin library and mock-server binary.
- Aggregates multiple references between the same two files into one arrow.
- Retains unresolved explicit project paths that could cross the boundary in
  JSON's `unresolved_project_paths`; the current snapshot has none.

This is a **source-dependency view, not a complete call graph**. It does not
perform Rust type inference, macro expansion, or runtime analysis. Inferred
receiver methods, generated dependencies, native dispatch, and external crates'
source files are not enumerated. Imports in local blocks are conservatively
combined within their enclosing module; ambiguous aliases can produce multiple
targets. An import alone counts as a dependency even if unused. Glob imports
alone do not imply an edge to every exported file.

## Regenerate

Requires Python 3.9+, Cargo/Rust, and Graphviz's `dot`. Cargo fetches the parser
dependencies on first use; its helper manifest/cache stays outside the repository.
The generator does not build the Windows plugin or change its dependencies.

From the repository root:

```sh
python3 scripts/chivalry2_graph.py
python3 -m unittest discover -s scripts -p test_chivalry2_graph.py
```

Templates and extraction code live in `scripts/chivalry2_graph*`.
