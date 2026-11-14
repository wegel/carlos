"""Keep the core's direct dependencies reviewed and every platform terminal-free."""

import importlib.util
from pathlib import Path
import unittest
import subprocess
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("core_dependencies", ROOT / "scripts/check-core-dependencies.py")
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


def graph(dependencies):
    """Build a resolved graph; names can differ from the Cargo package ids."""
    packages = [{"id": key, "name": name} for key, (name, _) in dependencies.items()]
    nodes = [{"id": key, "dependencies": children} for key, (_, children) in dependencies.items()]
    return {"packages": packages, "workspace_members": ["core"], "resolve": {"nodes": nodes}}


class CoreDependencyTests(unittest.TestCase):
    def test_reviewed_library_graph_passes(self):
        sample = graph({"core": ("carlos-core", ["json"]), "json": ("serde_json", [])})
        self.assertEqual(GUARD.check_graph(sample), [])

    def test_unused_direct_dependency_still_needs_review(self):
        sample = graph({"core": ("carlos-core", ["runtime"]), "runtime": ("tokio", [])})
        self.assertIn("carlos-core: unreviewed direct package tokio", GUARD.check_graph(sample))

    def test_alias_cannot_hide_a_terminal_or_audio_package(self):
        for name in GUARD.FORBIDDEN | {"ratatui-extra", "tui-extra"}:
            sample = graph({"core": ("carlos-core", ["json"]),
                            "json": ("serde_json", ["renamed-package"]),
                            "renamed-package": (name, [])})
            self.assertTrue(any("forbidden resolved package" in issue for issue in GUARD.check_graph(sample)))

    def test_target_only_and_dev_edges_are_traversed(self):
        sample = graph({"core": ("carlos-core", ["json"]),
                        "json": ("serde_json", ["win", "tests"]),
                        "win": ("crossterm", []), "tests": ("cpal", [])})
        issues = GUARD.check_graph(sample)
        self.assertIn("carlos-core: forbidden resolved package crossterm", issues)
        self.assertIn("carlos-core: forbidden resolved package cpal", issues)

    def test_unrelated_ui_member_does_not_pollute_the_core_graph(self):
        sample = graph({"core": ("carlos-core", []), "ui": ("carlos", ["draw"]),
                        "draw": ("ratatui", [])})
        sample["workspace_members"].append("ui")
        self.assertEqual(GUARD.check_graph(sample), [])

    def test_protocol_cannot_gain_io_even_through_an_alias(self):
        for name in ["tokio", "socket2", "carlos-core", "crossterm"]:
            sample = graph({"core": ("carlos-core", ["proto"]),
                            "proto": ("carlos-proto", ["json"]),
                            "json": ("serde_json", ["hidden"]), "hidden": (name, [])})
            self.assertIn(f"carlos-proto: unreviewed resolved package {name}", GUARD.check_graph(sample))

    def test_missing_core_member_cannot_pass_vacuously(self):
        sample = graph({"core": ("carlos", [])})
        self.assertTrue(GUARD.check_graph(sample))

    def test_fixture_helpers_cannot_become_a_default_feature_directly_or_indirectly(self):
        for defaults in [["test-support"], ["fixtures"]]:
            sample = graph({"core": ("carlos-core", [])})
            sample["packages"][0]["features"] = {
                "default": defaults, "fixtures": ["test-support"], "test-support": [],
            }
            self.assertTrue(any("fixture helpers are opt-in" in issue for issue in GUARD.check_graph(sample)))

    def test_daemon_cannot_hide_terminal_or_audio_dependencies_behind_an_alias(self):
        sample = graph({"core": ("carlos-core", []), "daemon": ("carlosd", ["hidden"]), "hidden": ("ratatui", [])})
        sample["workspace_members"].append("daemon")
        issues = GUARD.check_graph(sample)
        self.assertIn("carlosd: forbidden resolved package ratatui", issues)
        self.assertIn("carlosd: unreviewed direct package ratatui", issues)

    def test_daemon_direct_packages_need_review_even_when_unused(self):
        sample = graph({"core": ("carlos-core", []), "daemon": ("carlosd", ["runtime"]), "runtime": ("tokio", [])})
        sample["workspace_members"].append("daemon")
        self.assertIn("carlosd: unreviewed direct package tokio", GUARD.check_graph(sample))




class CargoMetadataTests(unittest.TestCase):
    def test_all_feature_and_platform_edges_are_requested_without_target_filter(self):
        fixture = '{"packages":[{"id":"core","name":"carlos-core"}],"workspace_members":["core"],"resolve":{"nodes":[{"id":"core","dependencies":[]}]}}'
        with patch.object(GUARD.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, fixture)) as run:
            self.assertEqual(GUARD.check_tree(ROOT), [])
        arguments = run.call_args.args[0]
        self.assertIn("--all-features", arguments)
        self.assertIn("--locked", arguments)
        self.assertNotIn("--filter-platform", arguments)

    def test_actual_workspace_keeps_the_core_and_protocol_dependency_rules(self):
        self.assertEqual(GUARD.check_tree(ROOT), [])


if __name__ == "__main__":
    unittest.main()
