# Rebuild the helper with a modified Bun runtime

Normal Carlos use needs only the two packaged executables and your installed
native Claude CLI. These optional rebuild steps need development tools.

`controller.js` contains the helper and its pinned JavaScript dependencies,
without Bun's native runtime. Keep its accompanying third-party notices.
Bun 1.3.10's upstream license explains the statically linked JavaScriptCore
library and its LGPL terms. The package includes that notice and library terms.

Use the exact runtime sources:

- [Bun 1.3.10](https://github.com/oven-sh/bun/tree/bun-v1.3.10).
- [Its WebKit source](https://github.com/oven-sh/WebKit/tree/4a6a32c32c11ffb9f5a94c310b10f50130bfe6de).
- [Bun's pinned build instructions](https://github.com/oven-sh/bun/blob/bun-v1.3.10/docs/project/contributing.mdx), including dependencies and the Windows guide linked there.

After installing those build dependencies, clone Bun and WebKit at those refs.
Inside the Bun checkout:

```sh
git checkout bun-v1.3.10
git clone https://github.com/oven-sh/WebKit vendor/WebKit
git -C vendor/WebKit checkout 4a6a32c32c11ffb9f5a94c310b10f50130bfe6de
bun run build:local
```

The pinned build guide places this runtime at `build/debug-local/bun-debug`
(`.exe` on Windows). Apply your library changes and rebuild it as needed.
Use a matching platform/architecture. Then use Bun 1.3.10's compiler with that
runtime, from the package's `relink` folder:

```sh
bun build --compile --compile-executable-path=/absolute/path/to/rebuilt/bun-debug \
  --no-compile-autoload-dotenv --no-compile-autoload-bunfig \
  --no-compile-autoload-tsconfig --no-compile-autoload-package-json \
  controller.js --outfile ../carlos-claude-controller
```

Use `../carlos-claude-controller.exe` on Windows. Run the package smoke test from
Carlos's source checkout against the rebuilt helper before replacing your copy.
The helper's sources, exact package versions, and lockfile are under
`helpers/claude-controller/` in that same Carlos source release. Native Claude and
its login are separate; none of these steps copies credentials or rebuilds Claude.
