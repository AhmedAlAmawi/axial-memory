"""Tests for the parts of axial-capture.py that don't need Rhino.

    python3 rhino/test_axial_capture.py
"""

import importlib.util
import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("axial_capture", os.path.join(HERE, "axial-capture.py"))
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)


class Fake(object):
    def __init__(self, name):
        self.name = name
        self.visible = True


class DataflowOrder(unittest.TestCase):
    def test_inputs_come_first_then_canvas_order(self):
        upstream = {"curve": ["points"], "points": ["slider"], "pipe": ["curve", "radius"]}
        position = {"slider": (0, 0), "radius": (0, 300), "points": (200, 0), "curve": (400, 0), "pipe": (600, 0), "note": (50, 900)}
        order = capture.dataflow_order(["pipe", "curve", "note", "radius", "points", "slider"], upstream, position)
        self.assertEqual(order, ["slider", "radius", "note", "points", "curve", "pipe"])

    def test_loops_keep_canvas_order(self):
        order = capture.dataflow_order(["b", "a"], {"a": ["b"], "b": ["a"]}, {"a": (0, 0), "b": (10, 0)})
        self.assertEqual(order, ["a", "b"])

    def test_unknown_sources_are_ignored(self):
        self.assertEqual(capture.dataflow_order(["a"], {"a": ["ghost"]}, {"a": (0, 0)}), ["a"])


class Sequence(unittest.TestCase):
    def test_each_picture_shows_one_stage_alone(self):
        objs = [Fake(n) for n in "abcd"]
        stages = [objs[0], objs[2]]
        seen = []

        def snap(stage):
            seen.append((stage.name, sorted(o.name for o in objs if o.visible)))
            return stage.name + ".png"

        results, stopped = capture.run_sequence(
            stages, objs, lambda o: setattr(o, "visible", False), lambda o: setattr(o, "visible", True), snap, lambda: False
        )
        self.assertFalse(stopped)
        self.assertEqual(seen, [("a", ["a"]), ("c", ["c"])])
        self.assertEqual([r[1] for r in results], ["a.png", "c.png"])

    def test_a_failed_picture_doesnt_stop_the_rest(self):
        objs = [Fake(n) for n in "ab"]

        def snap(stage):
            if stage.name == "a":
                raise RuntimeError("view gone")
            return "b.png"

        results, _ = capture.run_sequence(objs, objs, lambda o: setattr(o, "visible", False), lambda o: setattr(o, "visible", True), snap, lambda: False)
        self.assertEqual(results[0][2], "RuntimeError: view gone")
        self.assertEqual(results[1][1], "b.png")
        self.assertFalse(any(o.visible for o in objs))

    def test_escape_stops_between_pictures(self):
        objs = [Fake(n) for n in "abc"]
        calls = []
        results, stopped = capture.run_sequence(
            objs, objs, lambda o: None, lambda o: None, lambda s: calls.append(s.name), lambda: len(calls) == 2
        )
        self.assertTrue(stopped)
        self.assertEqual(calls, ["a", "b"])


class Files(unittest.TestCase):
    def test_safe_names(self):
        self.assertEqual(capture.safe_name('Panel: "a/b" <x>?'), "Panel_ _a_b_ _x__")
        self.assertEqual(capture.safe_name("   "), "step")
        self.assertEqual(len(capture.safe_name("x" * 200)), 70)

    def test_contact_sheet_lists_pictures_and_data_steps(self):
        manifest = {
            "captured_at": "2026-10-10 15:30",
            "definition": {"name": "palm <v4>", "path": "C:\\palm.gh"},
            "pictures": 1,
            "stopped": False,
            "steps": [
                {"step": 1, "picture": None, "name": "Number Slider", "nickname": "Count", "group": "Phyllotaxis",
                 "outputs": [{"name": "N", "type": "Number", "count": 1, "values": ["144"]}]},
                {"step": 2, "picture": "002 Pipe - Stalk.png", "name": "Pipe", "nickname": "Stalk", "group": None, "error": None},
            ],
        }
        html = capture.contact_sheet(manifest)
        self.assertIn('<img src="002 Pipe - Stalk.png"', html)
        self.assertIn("1 Number: 144", html)
        self.assertIn("palm &lt;v4&gt;", html)
        with tempfile.TemporaryDirectory() as d:
            capture.write_text(os.path.join(d, "index.html"), html)
            capture.write_text(os.path.join(d, "manifest.json"), json.dumps(manifest, ensure_ascii=False))
            self.assertTrue(os.path.getsize(os.path.join(d, "index.html")) > 0)

    def test_sha256_of_missing_file_is_none(self):
        self.assertIsNone(capture.file_sha256(None))
        self.assertIsNone(capture.file_sha256("/no/such/file.gh"))


if __name__ == "__main__":
    sys.exit(unittest.main())
