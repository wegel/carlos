# Third-party notices

Carlos and its helper have separate notices. The JavaScript folder contains the
licenses supplied by the packages actually bundled into the helper. Development
packages such as TypeScript and type declarations are excluded.

The Bun folder preserves upstream runtime/library notices, with pinned source
URLs and checksums in SOURCES.json. Some libraries apply only to particular
platforms. Bun 1.3.10 links JavaScriptCore/WebKit; the accompanying relink folder
contains the helper's standalone JavaScript bundle and concrete rebuild steps.
The upstream notice is reproduced verbatim, including its original formatting.
Use the pinned build guide in relink/README.md rather than the older commands
quoted in the upstream license notice.

The Rust folder contains the license files supplied by Cargo's resolved build
inputs for this target, plus pinned upstream workspace licenses omitted from
some published crates. Its inventory records each crate's version, declared
license, authors, and repository. Native Whisper's vendored license files are
included as well. The older RealFFT README and some objc2 workspace notices
declare MIT by reference; pinned standard MIT text accompanies those declarations.
Source URLs and checksums accompany the
additional upstream files. Carlos's own MIT license is at the package root.

The native Claude CLI, its optional SDK CLI packages, user settings, and account
credentials are not included in the package.
