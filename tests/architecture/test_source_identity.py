"""Keep the standard-library build script and helper packager's source markers identical."""

import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("source_identity", ROOT / "scripts/source-identity.py")
IDENTITY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IDENTITY)


def fixture(root):
    files = {
        "Cargo.toml": "workspace fixture", "Cargo.lock": "locked fixture",
        "src/main.rs": "terminal fixture", "crates/carlos-proto/Cargo.toml": "proto fixture",
        "crates/carlos-proto/src/lib.rs": "proto source",
        "helpers/claude-controller/src/main.ts": "helper fixture",
    }
    for name in ("package.json", "bun.lock", "bunfig.toml", "tsconfig.json"):
        files["helpers/claude-controller/" + name] = name
    for name, content in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
    shutil.copy2(ROOT / "crates/carlos-proto/build.rs", root / "crates/carlos-proto/build.rs")


class SourceIdentityTests(unittest.TestCase):
    def test_rust_and_python_agree_independent_of_absolute_directory(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            first, second = root / "first", root / "a longer second path"
            fixture(first)
            fixture(second)
            compiler = shutil.which("rustc")
            self.assertIsNotNone(compiler, "Cargo checks require rustc")
            tool = root / "fingerprint"
            subprocess.run([compiler, "--edition=2021", str(ROOT / "crates/carlos-proto/build.rs"),
                            "-o", str(tool)], check=True, capture_output=True)
            markers = []
            for source in (first, second):
                output = subprocess.check_output([str(tool)], text=True, env={**os.environ,
                    "CARGO_MANIFEST_DIR": str(source / "crates/carlos-proto")})
                rust = output.split("CARLOS_SOURCE_ID=", 1)[1].strip()
                self.assertEqual(rust, IDENTITY.source_identity(source))
                markers.append(rust)
            self.assertEqual(markers[0], markers[1])
            (second / "src/main.rs").write_text("changed source")
            self.assertNotEqual(markers[0], IDENTITY.source_identity(second))

    def test_relative_names_and_helper_or_lock_changes_affect_the_marker(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            fixture(root)
            original = IDENTITY.source_identity(root)
            source = root / "src/main.rs"
            source.rename(source.with_name("renamed.rs"))
            self.assertNotEqual(original, IDENTITY.source_identity(root))
            source.with_name("renamed.rs").rename(source)
            for name in ("Cargo.lock", "helpers/claude-controller/src/main.ts"):
                path = root / name
                before = path.read_bytes()
                path.write_bytes(before + b" changed")
                self.assertNotEqual(original, IDENTITY.source_identity(root))
                path.write_bytes(before)


if __name__ == "__main__":
    unittest.main()
