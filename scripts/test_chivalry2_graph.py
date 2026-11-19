import unittest

from chivalry2_graph import graph


class BoundaryGraphTests(unittest.TestCase):
    def setUp(self):
        self.data = {"modules": [], "imports": [], "references": []}
        self.module(["app"], "src/lib.rs", ["outside"])
        self.module(["app", "chivalry2"], "src/chivalry2/mod.rs")
        self.module(["app", "chivalry2", "owner"], "src/chivalry2/owner.rs", ["Native"])
        self.module(["app", "chivalry2", "unused"], "src/chivalry2/unused.rs", ["Unused"])
        self.module(["app", "consumer"], "src/consumer.rs")
        self.module(["app", "helpers"], "src/helpers.rs", ["helper"])
        self.ref(["app"], "src/lib.rs", ["self", "chivalry2"])

    def module(self, module, file, definitions=()):
        self.data["modules"].append({"module": module, "file": file, "definitions": definitions})

    def use(self, module, path, alias=None, glob=False):
        self.data["imports"].append({
            "module": module, "file": "src/consumer.rs" if module[-1] == "consumer" else "src/chivalry2/mod.rs",
            "path": path, "alias": alias or path[-1], "glob": glob, "line": 1,
        })

    def ref(self, module, file, path):
        self.data["references"].append({"module": module, "file": file, "path": path, "line": 7, "kind": "path"})

    def edges(self):
        return {(e["source"], e["target"]) for e in graph(self.data)["edges"]}

    def test_globs_resolve_used_owner_not_every_export(self):
        self.use(["app", "chivalry2"], ["self", "owner"], glob=True)
        self.use(["app", "chivalry2"], ["self", "unused"], glob=True)
        self.use(["app", "consumer"], ["crate", "chivalry2"], glob=True)
        self.ref(["app", "consumer"], "src/consumer.rs", ["Native", "method"])
        edges = self.edges()
        self.assertIn(("src/consumer.rs", "src/chivalry2/owner.rs"), edges)
        self.assertNotIn(("src/consumer.rs", "src/chivalry2/unused.rs"), edges)

    def test_alias_and_super_references_point_outward(self):
        self.use(["app", "chivalry2"], ["crate", "helpers", "helper"], alias="alias")
        self.ref(["app", "chivalry2"], "src/chivalry2/mod.rs", ["alias"])
        self.ref(["app", "chivalry2", "owner"], "src/chivalry2/owner.rs", ["super", "super", "outside"])
        edges = self.edges()
        self.assertIn(("src/chivalry2/mod.rs", "src/helpers.rs"), edges)
        self.assertIn(("src/chivalry2/owner.rs", "src/lib.rs"), edges)

    def test_same_side_and_external_references_are_excluded(self):
        self.ref(["app", "chivalry2"], "src/chivalry2/mod.rs", ["crate", "chivalry2", "owner", "Native"])
        self.ref(["app", "consumer"], "src/consumer.rs", ["crate", "helpers", "helper"])
        self.ref(["app", "chivalry2"], "src/chivalry2/mod.rs", ["std", "mem", "size_of"])
        self.assertEqual(self.edges(), {("src/lib.rs", "src/chivalry2/mod.rs")})

    def test_macro_references_use_definition_file(self):
        self.module(["macros"], "unchained-macros/src/lib.rs", ["native_binding"])
        self.data["modules"][-1]["macros"] = ["native_binding"]
        self.ref(["app", "chivalry2"], "src/chivalry2/mod.rs", ["native_binding"])
        self.assertIn(("src/chivalry2/mod.rs", "unchained-macros/src/lib.rs"), self.edges())


if __name__ == "__main__":
    unittest.main()
