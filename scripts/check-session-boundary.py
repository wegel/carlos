#!/usr/bin/env python3
"""Reject terminal dependencies in the files that will move into carlos-core."""

from pathlib import Path
import posixpath
import re
import sys
import tomllib


EXTERNAL_TESTS = {
    "src/tests/native_form_tests.rs",
    "src/tests/codex_connection_tests.rs",
    "src/tests/protocol_transport_tests.rs",
    "src/tests/protocol_history_transport_tests.rs",
    "src/tests/protocol_stop_tests.rs",
    "src/tests/protocol_capability_tests.rs",
    "src/tests/protocol_history_tests.rs",
    "src/tests/codex_history_file_tests.rs",
}
CRATES = {
    "anyhow", "base64", "blake3", "libc", "rusqlite", "serde_json", "shlex",
    "tempfile", "unicode_segmentation", "unicode_width", "uuid", "jsonschema",
    "url", "mio", "socket2", "tungstenite",
}
TERMINAL = {
    "ratatui", "ratatui_core", "ratatui_interact", "ratatui_textarea", "crossterm",
    "ansi_to_tui", "tui_markdown", "termion", "tui",
}
CLIENT_TYPES = {"AppState", "ViewState", "TextArea", "Widget", "StatefulWidget", "Terminal"}
STANDARD = {"std", "core", "alloc", "crate", "self", "super", "Self"}
STANDARD.update({"str", "char", "bool", "usize", "isize", "f32", "f64"})
STANDARD.update(f"{sign}{bits}" for sign in ("u", "i") for bits in (8, 16, 32, 64, 128))


def block_comment_end(source, at):
    """Skip nested Rust comments without interpreting quotes in their contents."""
    depth = 1
    at += 2
    while at < len(source) and depth:
        if source.startswith("/*", at):
            depth += 1
            at += 2
        elif source.startswith("*/", at):
            depth -= 1
            at += 2
        else:
            at += 1
    if depth:
        raise ValueError("unclosed block comment")
    return at


def tokens(source):
    """Keep Rust code and attribute strings; skip comments and literal contents."""
    result = []
    at = 0
    while at < len(source):
        if source[at].isspace():
            at += 1
            continue
        if source.startswith("//", at):
            end = source.find("\n", at)
            at = len(source) if end < 0 else end
            continue
        if source.startswith("/*", at):
            at = block_comment_end(source, at)
            continue
        raw = re.match(r'(?:br|cr|r)(#*)"', source[at:])
        if raw:
            closing = '"' + raw[1]
            end = source.find(closing, at + len(raw[0]))
            if end < 0:
                raise ValueError("unclosed raw string")
            result.append(("string", source[at + len(raw[0]):end], at))
            at = end + len(closing)
            continue
        quoted = re.match(r'(?:b|c)?"', source[at:])
        if quoted:
            start = at
            at += len(quoted[0])
            body = at
            while at < len(source) and source[at] != '"':
                at += 2 if source[at] == "\\" else 1
            if at >= len(source):
                raise ValueError("unclosed string")
            result.append(("string", source[body:at], start))
            at += 1
            continue
        char = re.match(r"b?'(?:\\(?:u\{[^}]+\}|x[0-9A-Fa-f]{2}|.)|[^'\\\n])'", source[at:])
        if char:
            at += len(char[0])
            continue
        word = re.match(r"(?:r#)?[^\W\d]\w*", source[at:])
        if word:
            result.append(("id", word[0].removeprefix("r#"), at))
            at += len(word[0])
            continue
        punctuation = "::" if source.startswith("::", at) else source[at]
        result.append(("code", punctuation, at))
        at += len(punctuation)
    return result


def core_files(files):
    """Enumerate exact files, including undeclared files under each future-core root."""
    roots = {"session", "backend", "claude_backend", "grok_backend", "summaries", "native_form"}
    roots.update(Path(name).stem for name in files if re.fullmatch(r"src/(?:protocol|codex|native_form)[^/]*\.rs", name))
    return {name for name in files if name.endswith(".rs") and any(
        name == f"src/{root}.rs" or name.startswith(f"src/{root}/") for root in roots
    )}


def module_children(stream, source_name):
    """Follow all cfg branches, inline modules and explicit path attributes."""
    owner = Path(source_name)
    normal = owner.parent if owner.name == "mod.rs" else owner.with_suffix("")
    values = [token[1] if token[0] != "string" else None for token in stream]

    def scope(start, end, directory, attribute_directory):
        at = start
        paths = []
        while at < end:
            if values[at] == "#" and at + 1 < end and values[at + 1] == "[":
                depth, stop = 1, at + 2
                while stop < end and depth:
                    depth += (values[stop] == "[") - (values[stop] == "]")
                    stop += 1
                for i in range(at + 2, stop - 2):
                    if values[i:i + 2] == ["path", "="]:
                        if stream[i + 2][0] != "string":
                            raise ValueError("unchecked module path")
                        paths.append(str(attribute_directory / stream[i + 2][1]))
                at = stop
                continue
            if values[at] == "mod" and at + 2 < end and stream[at + 1][0] == "id":
                name, form = values[at + 1:at + 3]
                if form == ";":
                    yield (paths or [str(directory / f"{name}.rs"), str(directory / name / "mod.rs")], bool(paths))
                    paths = []
                    at += 3
                    continue
                if form == "{":
                    if paths:
                        raise ValueError("path attribute on inline module needs an explicit check")
                    depth, stop = 1, at + 3
                    while stop < end and depth:
                        depth += (values[stop] == "{") - (values[stop] == "}")
                        stop += 1
                    yield from scope(at + 3, stop - 1, directory / name, directory / name)
                    paths = []
                    at = stop
                    continue
                raise ValueError("unchecked module declaration")
            if values[at] == "pub":
                at += 1
                if at < end and values[at] == "(":
                    while at < end and values[at] != ")":
                        at += 1
                    at += 1
                continue
            # An attribute applies to this item, not a later module in its body.
            if values[at] not in {"pub", "(", ")", "crate", "super", "self", "in"}:
                paths = []
            at += 1

    yield from scope(0, len(stream), normal, owner.parent)


def use_names(code):
    """Collect names brought into scope, including nested use trees and aliases."""
    found = set()
    for match in re.finditer(r"\b(?:use|extern\s+crate)\s+([^;]+);", code):
        words = re.findall(r"[A-Za-z_]\w*", match[1])
        found.update(words)
    return found


def imports(stream):
    """Expand use trees so crate::{app as ui} cannot hide a client root."""
    values = [t[1] if t[0] != "string" else None for t in stream]
    output = []

    def tree(at, prefix):
        path = list(prefix)
        while at < len(values):
            word = values[at]
            if word == "{":
                at += 1
                while at < len(values) and values[at] != "}":
                    at = tree(at, path)
                    if at < len(values) and values[at] == ",":
                        at += 1
                return at + 1
            if word in {"as", ",", ";", "}"}:
                if path:
                    binding = values[at + 1] if word == "as" else path[-1]
                    output.append((path, binding))
                if word == "as":
                    return at + 2
                return at
            if word != "::":
                path.append(word)
            at += 1
        return at

    for at, value in enumerate(values):
        if value == "use":
            tree(at + 1, [])
        elif values[at:at + 2] == ["extern", "crate"] and at + 2 < len(values):
            alias = values[at + 4] if values[at + 3:at + 4] == ["as"] else values[at + 2]
            output.append(([values[at + 2]], alias))
    return output


def session_aliases(codes):
    """Follow SessionState imports, re-exports and type aliases to a fixed point."""
    aliases = {"SessionState"}
    # Include aliases exported through other client files, not only direct imports.
    changed = True
    while changed:
        changed = False
        for code in codes.values():
            for match in re.finditer(r"\b(?:use|type)\s+([^;]+);", code):
                words = re.findall(r"[A-Za-z_]\w*", match[1])
                if any(word in aliases for word in words):
                    names = set(re.findall(r"\bas\s+(\w+)", match[1]))
                    if match[0].startswith("type ") and words:
                        names.add(words[0])
                    if not names <= aliases:
                        aliases.update(names)
                        changed = True
    return aliases


def check_impls(codes):
    """Keep inherent session methods under the session's private owner."""
    aliases = session_aliases(codes)
    issues = []
    for name, code in codes.items():
        if not name.startswith("src/session/"):
            for header in re.findall(r"\bimpl\b([^{}]+)\{", code):
                if aliases.intersection(re.findall(r"[A-Za-z_]\w*", header)):
                    issues.append(f"{name}: SessionState impl belongs under src/session/")
    return issues


def check_paths(name, stream, known, dependencies, root_names):
    """Check path heads outside use declarations, including macro paths."""
    issues = []
    declared = known
    for i, token in enumerate(stream[:-1]):
        if token[0] != "id" or stream[i + 1][1] != "::":
            continue
        if i and stream[i - 1][1] in {"::", "."}:
            continue
        head = token[1]
        if head == "crate" and i + 2 < len(stream):
            if stream[i + 2][1] != "{" and stream[i + 2][1] not in root_names:
                issues.append(f"{name}: client or unchecked crate path {stream[i + 2][1]}")
        elif head in dependencies and dependencies[head] not in CRATES:
            issues.append(f"{name}: unapproved external crate {head}")
        elif head not in STANDARD | CRATES | root_names | declared and not head[0].isupper():
            issues.append(f"{name}: unchecked path root {head}")
    return issues


def check_imports(name, stream, defined, dependencies, root_names):
    """Admit only reviewed crates, checked local modules and their aliases."""
    issues = []
    pending_imports = imports(stream)
    approved = {head for head, package in dependencies.items() if package in CRATES}
    known = STANDARD | CRATES | root_names | defined | approved
    while pending_imports:
        remaining = []
        for path, binding in pending_imports:
            if path[0] in known:
                known.add(binding)
            else:
                remaining.append((path, binding))
        if len(remaining) == len(pending_imports):
            for path, _ in remaining:
                issues.append(f"{name}: unchecked import root {path[0]}")
            break
        pending_imports = remaining
    for path, _ in imports(stream):
        head = path[0]
        if head == "crate" and len(path) > 1 and path[1] not in root_names:
            issues.append(f"{name}: client or unchecked crate path {path[1]}")
        if head in dependencies and dependencies[head] not in CRATES:
            issues.append(f"{name}: unapproved external crate {head}")
    return issues


def check_file(name, stream, code, root_names, dependencies):
    """Check every code identifier, dependency path and imported alias."""
    ids = {token[1] for index, token in enumerate(stream) if token[0] == "id"
           and not (index and stream[index - 1][1] == "'")}
    issues = [f"{name}: terminal dependency {identifier}"
              for identifier in sorted(ids.intersection(TERMINAL | CLIENT_TYPES))]
    if re.search(r"\binclude\s*!", code):
        issues.append(f"{name}: include! bypasses the checked module tree")
    defined = set(re.findall(r"\b(?:mod|struct|enum|trait|type|fn|const|static)\s+(\w+)", code))
    issues.extend(check_paths(name, stream, defined | use_names(code), dependencies, root_names))
    issues.extend(check_imports(name, stream, defined, dependencies, root_names))
    return issues


def checked_children(name, stream, files, allowed, external_tests):
    """Reject undeclared files outside the exact future-core and test roots."""
    issues, reached = [], []
    try:
        children = list(module_children(stream, name))
    except ValueError as error:
        return [f"{name}: {error}"], []
    for choices, explicit in children:
        # Resolve '..' without touching the host filesystem in sample tests.
        import posixpath
        choices = [posixpath.normpath(path) for path in choices]
        existing = [path for path in choices if path in files]
        if not existing:
            issues.append(f"{name}: missing checked module {' or '.join(choices)}")
        elif not explicit and len(existing) > 1:
            issues.append(f"{name}: ambiguous module {' or '.join(existing)}")
        for child in existing:
            if child not in allowed and child not in external_tests:
                issues.append(f"{name}: module escapes future core: {child}")
            else:
                reached.append(child)
    return issues, reached


def check_sources(files, dependencies=None, external_tests=EXTERNAL_TESTS):
    """Check a path-to-source map so tests need neither Cargo nor a Rust parser."""
    issues, lexed = [], {}
    for name, source in files.items():
        try:
            lexed[name] = tokens(source)
        except ValueError as error:
            issues.append(f"{name}: {error}")
    codes = {name: " ".join(t[1] for t in ts if t[0] != "string") for name, ts in lexed.items()}
    allowed = core_files(files)
    root_names = {Path(name).parts[1].removesuffix(".rs") for name in allowed}
    issues.extend(check_impls(codes))
    pending, visited = list(allowed), set()
    while pending:
        name = pending.pop()
        if name in visited or name not in lexed:
            continue
        visited.add(name)
        issues.extend(check_file(name, lexed[name], codes[name], root_names, dependencies or {}))
        child_issues, reached = checked_children(name, lexed[name], files, allowed, external_tests)
        issues.extend(child_issues)
        pending.extend(reached)
    return sorted(set(issues))


def check_tree(root):
    files = {str(path.relative_to(root)): path.read_text() for path in (root / "src").rglob("*.rs")}
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    dependencies = {}
    scopes = [manifest, *manifest.get("target", {}).values()]
    for scope in scopes:
        for group in ("dependencies", "dev-dependencies", "build-dependencies"):
            for name, spec in scope.get(group, {}).items():
                package = spec.get("package", name) if isinstance(spec, dict) else name
                dependencies[name.replace("-", "_")] = package.replace("-", "_")
    return check_sources(files, dependencies)


if __name__ == "__main__":
    errors = check_tree(Path(__file__).resolve().parents[1])
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    print("Session boundary: all future-core Rust files and declared tests passed")
