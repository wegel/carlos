"""Exercise dependency checks with small source trees and the actual repository."""

import importlib.util
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("session_boundary", ROOT / "scripts/check-session-boundary.py")
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class SessionBoundaryTests(unittest.TestCase):
    def assert_rejected(self, source, fragment, extras=None, dependencies=None):
        files = {"src/session/mod.rs": source, **(extras or {})}
        errors = GUARD.check_sources(files, dependencies)
        self.assertTrue(errors, files)
        self.assertTrue(any(fragment in error for error in errors), errors)
        return errors

    def test_direct_terminal_import_names_its_file(self):
        for crate in GUARD.TERMINAL:
            self.assert_rejected(f"use {crate}::Widget;", "src/session/mod.rs")

    def test_alias_reexport_macro_and_extern_paths_are_code(self):
        for source in [
            "use ratatui as screen; screen::draw();",
            "pub use crossterm::event as keys;",
            "ratatui::make_widget!();",
            "extern crate tui as screen;",
            "use r#ratatui::Widget;",
        ]:
            self.assert_rejected(source, "terminal dependency")

    def test_client_roots_and_widget_api_are_rejected(self):
        for root in ["app", "event", "theme", "clipboard", "dictation", "cli"]:
            self.assert_rejected(f"pub use crate::{root}::State as Facts;", root)
        self.assert_rejected("fn adopt(view: &ViewState) {}", "ViewState")
        self.assert_rejected("struct Editor { value: TextArea }", "TextArea")

    def test_local_child_and_inline_child_are_followed_in_all_cfg_branches(self):
        errors = self.assert_rejected("#[cfg(test)] mod checks;", "src/session/checks.rs", {
            "src/session/checks.rs": "use ratatui as screen;",
        })
        self.assertTrue(any("terminal dependency" in error for error in errors))
        self.assert_rejected("mod nested { #[cfg(any())] mod leaf; }", "src/session/nested/leaf.rs", {
            "src/session/nested/leaf.rs": "crate::event::poll();",
        })

    def test_explicit_path_child_cannot_hide_terminal_imports(self):
        self.assert_rejected('#[path = "child.rs"] mod private;', "src/session/child.rs", {
            "src/session/child.rs": "use termion::input;",
        })
        self.assert_rejected('mod nested { #[path = "child.rs"] mod hidden; }', "src/session/nested/child.rs", {
            "src/session/nested/child.rs": "use crossterm::event;",
        })

    def test_external_child_requires_an_exact_allowlist_entry(self):
        files = {"src/session/mod.rs": '#[path = "../tests/other.rs"] mod other;',
                 "src/tests/other.rs": "fn clean() {}"}
        self.assertTrue(any("escapes" in error for error in GUARD.check_sources(files)))
        self.assertEqual(GUARD.check_sources(files, external_tests={"src/tests/other.rs"}), [])
        files["src/tests/other.rs"] = "use crate::app::test_support;"
        self.assertTrue(any("src/tests/other.rs" in error for error in GUARD.check_sources(
            files, external_tests={"src/tests/other.rs"})))

    def test_missing_ambiguous_and_unchecked_modules_fail_closed(self):
        self.assert_rejected("mod missing;", "missing checked module")
        self.assert_rejected("mod child;", "ambiguous module", {
            "src/session/child.rs": "", "src/session/child/mod.rs": "",
        })
        self.assert_rejected('#[path = concat!("a", ".rs")] mod child;', "unchecked module path")
        self.assert_rejected('include!("../app.rs");', "include!")

    def test_inline_path_attributes_cannot_redirect_a_clean_child_elsewhere(self):
        self.assert_rejected('#[path = "../app"] mod nested { mod child; }', "explicit check", {
            "src/session/nested/child.rs": "", "src/app/child.rs": "use ratatui::Terminal;",
        })

    def test_undeclared_core_file_is_checked_too(self):
        self.assert_rejected("fn clean() {}", "src/session/unused.rs", {
            "src/session/unused.rs": "use ratatui::Terminal;",
        })

    def test_external_dependency_and_cargo_alias_need_review(self):
        self.assert_rejected("use tokio as runtime;", "unapproved external crate", dependencies={"tokio": "tokio"})
        self.assert_rejected("drawkit::draw();", "unapproved external crate", dependencies={"drawkit": "ratatui"})
        self.assert_rejected("extern crate serde;", "unapproved external crate", dependencies={"serde": "serde"})
        self.assertEqual(GUARD.check_sources({"src/session/mod.rs": "use anyhow::Result;"}, {"anyhow": "anyhow"}), [])

    def test_grouped_imports_and_restricted_visibility_do_not_hide_paths(self):
        self.assert_rejected("pub use crate::{app as ui, session};", "app")
        self.assert_rejected('#[path = "../app.rs"] pub(in crate::session) mod child;', "escapes", {
            "src/app.rs": "use ratatui::Terminal;", "src/session/child.rs": "",
        })
        self.assertEqual(GUARD.check_sources({"src/session/mod.rs": "use crate::{session, backend};",
                                             "src/backend.rs": ""}), [])

    def test_unicode_module_names_and_unknown_imports_cannot_skip_checks(self):
        self.assert_rejected('#[path = "../other.rs"] mod caché;', "escapes", {
            "src/other.rs": "use ratatui::Widget;",
        })
        self.assert_rejected("use unchecked_crate as alias; alias::run();", "unchecked import root")
        self.assertEqual(GUARD.check_sources({"src/session/mod.rs":
            "use std::sync::mpsc as queue; use queue::Sender;"}), [])

    def test_target_specific_dependency_aliases_need_review(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src/session").mkdir(parents=True)
            (root / "src/session/mod.rs").write_text("use displaykit as render;")
            (root / "Cargo.toml").write_text(
                '[target.\'cfg(windows)\'.dependencies]\ndisplaykit = { package = "ratatui", version = "1" }\n')
            self.assertTrue(any("unapproved external crate displaykit" in error
                                for error in GUARD.check_tree(root)))

    def test_comments_literals_chars_and_lifetimes_do_not_create_dependencies(self):
        source = '''
            // use ratatui::Widget;
            /* nested /* crate::app::State */ crossterm */
            const DATA: &str = r###"ratatui::Frame; \\" ; crate::event::poll()"###;
            const BYTES: &[u8] = br#"crate::app::AppState"#;
            const CONTROL: &str = "\\x1b[31mratatui::Frame\\x1b[0m";
            fn text<'a>(value: &'a str) -> &'a str { let c = '\\''; value }
            fn named_lifetime<'ratatui>(value: &'ratatui str) -> &'ratatui str { value }
        '''
        self.assertEqual(GUARD.check_sources({"src/session/mod.rs": source}), [])

    def test_session_impl_cannot_escape_through_import_or_type_alias(self):
        for outside in [
            "impl crate::session::state::SessionState { fn f() {} }",
            "use crate::session::state::SessionState as Owner; impl Owner {}",
            "type Owner = crate::session::state::SessionState; impl Owner {}",
            "use crate::bridge::Owner as Local; impl Local {}",
        ]:
            errors = GUARD.check_sources({"src/session/mod.rs": "", "src/app/test.rs": outside,
                "src/bridge.rs": "pub use crate::session::state::SessionState as Owner;"})
            self.assertTrue(any("src/app/test.rs: SessionState impl" in error for error in errors), errors)

    def test_real_repository(self):
        self.assertEqual(GUARD.check_tree(ROOT), [])

    def test_library_root_and_unlisted_modules_are_checked(self):
        files = {"src/lib.rs": "pub mod new_backend;",
                 "src/new_backend.rs": "use crossterm::event;"}
        self.assertTrue(any("terminal dependency" in error for error in GUARD.check_sources(files)))
        files["src/new_backend.rs"] = "use anyhow::Result;"
        self.assertEqual(GUARD.check_sources(files, {"anyhow": "anyhow"}), [])

    def test_workspace_checks_the_actual_library_not_the_old_roots(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / "crates/carlos-core"
            (library / "src").mkdir(parents=True)
            (library / "Cargo.toml").write_text('[package]\nname = "carlos-core"\n')
            (library / "src/lib.rs").write_text("use ratatui::Terminal;")
            (root / "src").mkdir()
            (root / "src/main.rs").write_text("fn main() {}")
            self.assertTrue(any("terminal dependency" in error for error in GUARD.check_tree(root)))


if __name__ == "__main__":
    unittest.main()
