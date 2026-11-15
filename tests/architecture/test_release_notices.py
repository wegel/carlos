"""Verify Cargo's shared license files survive the workspace package step."""

import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("package_release", ROOT / "scripts/package-release.py")
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


class ReleaseNoticeTests(unittest.TestCase):
    def test_package_requires_matching_protocol_version_source_and_helper_provenance(self):
        identity = {"protocol":1, "version":"fixture", "source":"a" * 32}
        helper = {"sourceId":identity["source"]}
        def check(other, metadata=helper):
            replies = [SimpleNamespace(stdout=json.dumps(value)) for value in (identity, other)]
            with patch.object(PACKAGE.subprocess, "run", side_effect=replies), \
                 patch.object(PACKAGE.subprocess, "check_output", return_value=identity["source"]):
                return PACKAGE.matching_build(Path("carlos"), Path("carlosd"), metadata)
        self.assertEqual(check(identity), identity)
        for change in ({"protocol":2}, {"version":"different"}, {"source":"b" * 32}):
            with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, "same current source"):
                check({**identity, **change})
        with self.assertRaisesRegex(RuntimeError, "Helper source"):
            check(identity, {"sourceId":"b" * 32})

    def test_daemon_only_dependencies_are_included_but_dev_dependencies_are_not(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            packages = []
            for name in ("carlos", "carlosd", "daemon-library", "dev-only"):
                folder = root / name
                folder.mkdir()
                (folder / "LICENSE").write_text(name)
                packages.append({"id": name, "name": name, "version": "0.1.0",
                    "manifest_path": str(folder / "Cargo.toml"), "license_file": None,
                    "license": "MIT", "repository": None, "authors": []})
            metadata = {"packages": packages, "resolve": {"root": "carlos", "nodes": [
                {"id": "carlos", "deps": []},
                {"id": "carlosd", "deps": [
                    {"pkg": "daemon-library", "dep_kinds": [{"kind": None}]},
                    {"pkg": "dev-only", "dep_kinds": [{"kind": "dev"}]}]},
                {"id": "daemon-library", "deps": []}, {"id": "dev-only", "deps": []}]}}
            with patch.object(PACKAGE.subprocess, "check_output", return_value=json.dumps(metadata)):
                rows = PACKAGE.rust_notices(root / "notices", "fixture-target")
            self.assertEqual({row["name"] for row in rows}, {"carlos", "carlosd", "daemon-library"})

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
