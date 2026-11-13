"""Verify Cargo's shared license files survive the workspace package step."""

import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("package_release", ROOT / "scripts/package-release.py")
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


class ReleaseNoticeTests(unittest.TestCase):
    def test_shared_license_file_is_copied_for_a_workspace_library(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / "crates/carlos-core"
            library.mkdir(parents=True)
            (root / "LICENSE").write_text("Carlos shared license fixture\n")
            metadata = {
                "resolve": {"root": "core", "nodes": [{"id": "core", "deps": []}]},
                "packages": [{"id": "core", "name": "carlos-core", "version": "0.1.0",
                              "manifest_path": str(library / "Cargo.toml"),
                              "license_file": "../../LICENSE", "license": None,
                              "repository": None, "authors": []}],
            }
            with patch.object(PACKAGE.subprocess, "check_output", return_value=json.dumps(metadata)):
                rows = PACKAGE.rust_notices(root / "notices", "fixture-target")
            self.assertEqual(rows[0]["name"], "carlos-core")
            copied = root / "notices/carlos-core-0.1.0/LICENSE"
            self.assertEqual(copied.read_text(), (root / "LICENSE").read_text())


if __name__ == "__main__":
    unittest.main()
