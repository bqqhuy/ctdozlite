"""Offline tests for roadmap provenance, namespaces and rebuild safety."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("roadmap", Path(__file__).parents[1] / "graphify_roadmap.py")
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class RoadmapTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.manifest = {
            "schema_version": 1, "roadmap": "docs/roadmap.md", "gap_source": "docs/gaps.md",
            "releases": {"v0.008-arm": {"blocks": ["R8"], "status": "pending", "specs": ["demo", "other"]}},
            "carryover_specs": {}, "evidence": [], "bindings": [],
            "blocks": {"R8": [f"{prefix}{i:02}" for prefix, stop in [("A", 20), ("B", 12), ("C", 4)] for i in range(1, stop + 1)]}
        }
        self.write("docs/roadmap.md", "# Roadmap\n\n[Spec](../specs/demo/spec.md)\n")
        self.write("docs/gaps.md", "# Gaps\n" + "\n".join(
            f"| {x} | Description | source | baseline |" if x[0] != "C" else f"- {x} — Description"
            for x in self.manifest["blocks"]["R8"]))
        self.write("docs/releases/v0.008-arm-plan.md", "# Release\n")
        self.write("docs/releases/v0.008-arm-progress.md", "# Pending device\n")
        for name in ["demo", "other"]:
            self.write(f"specs/{name}/spec.md", "# Spec\n| S-01 | Requirement |\n")
            self.write(f"specs/{name}/tasks.md", "# Tasks\n- [x] [S-T001] [S-01] Code model only\n- [ ] [S-T090] [S-01] Device pending\n")
            self.write(f"specs/{name}/plan.md", "# Plan\n")
            self.write(f"specs/{name}/verification.md", "# No device run\n")
        self.graph = {"graph": {}, "nodes": [{"id": "ast-symbol", "label": "example()", "_origin": "ast"}], "links": []}

    def write(self, path, content):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(content, encoding="utf-8")

    def test_rebuild_is_idempotent_and_does_not_promote_device_acceptance(self):
        graph, stats = module.enrich(self.root, self.graph, self.manifest)
        rebuilt, repeated = module.enrich(self.root, graph, self.manifest)
        self.assertEqual(graph, rebuilt)
        self.assertEqual(stats, repeated)
        tasks = [n for n in graph["nodes"] if n.get("kind") == "task"]
        self.assertEqual(sum(n["status"] == "open" for n in tasks), 2)
        self.assertEqual(len({n["id"] for n in tasks}), 4)
        self.assertIn(self.graph["nodes"][0], rebuilt["nodes"])

    def test_deleted_or_renamed_task_drops_its_previous_edges(self):
        graph, _ = module.enrich(self.root, self.graph, self.manifest)
        self.write("specs/demo/tasks.md", "# Tasks\n- [ ] [S-T091] [S-01] New task\n")
        updated, _ = module.enrich(self.root, graph, self.manifest)
        old = "roadmap:task:demo:S-T090"
        self.assertNotIn(old, {n["id"] for n in updated["nodes"]})
        self.assertFalse(any(e["source"] == old or e["target"] == old for e in updated["links"]))

    def test_missing_gap_or_test_binding_rejects_incomplete_graph(self):
        invalid = copy.deepcopy(self.manifest)
        invalid["blocks"]["R8"].remove("A20")
        with self.assertRaises(ValueError):
            module.enrich(self.root, self.graph, invalid)
        invalid = copy.deepcopy(self.manifest)
        invalid["bindings"] = [{"spec": "demo", "requirements": ["S-01"], "code": [], "tests": ["src/missing.rs::test"]}]
        with self.assertRaises(ValueError):
            module.enrich(self.root, self.graph, invalid)
        self.assertEqual(len(self.graph["nodes"]), 1)

    def test_source_escape_and_unlisted_private_files_are_not_imported(self):
        self.write("session.json", '{"secret":"do-not-read"}')
        graph, _ = module.enrich(self.root, self.graph, self.manifest)
        self.assertFalse(any(n.get("source_file") == "session.json" for n in graph["nodes"]))
        with self.assertRaises(ValueError):
            module.read_source(self.root, "../outside.md")
        with self.assertRaises(ValueError):
            module.read_source(self.root, "session.json")


if __name__ == "__main__":
    unittest.main()
