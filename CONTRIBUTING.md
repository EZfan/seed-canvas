# Contributing to seed-canvas

Thank you for your interest in contributing! seed-canvas is a community
project, and we welcome bug reports, feature requests, documentation
improvements, templates, and code.

## Code of Conduct

By participating, you agree to abide by the
[Code of Conduct](./CODE_OF_CONDUCT.md). Be kind, be patient, assume good faith.

## Ways to Contribute

| Type | Where to start |
| --- | --- |
| 🐛 Bug reports | [Open an issue](../../issues/new?template=bug.yml) |
| ✨ Feature requests | [Open an issue](../../issues/new?template=feature.yml) |
| 📖 Documentation | Edit files under `docs/` or the README and open a PR |
| 🎨 Templates | Add an example under `examples/` and reference it in `registry/index.json` |
| 🔧 Code | Pick a `good first issue` from the issue list |

## Local Development Setup

### Prerequisites

- **Node.js ≥ 20** (we test on 20, 22, 24)
- **pnpm ≥ 9** (`corepack enable && corepack prepare [email protected] --activate`)
- **Python 3.10+** with [`uv`](https://github.com/astral-sh/uv) (only needed for the
  Python SDK)

### Clone and Install

```bash
git clone https://github.com/EZfan/seed-canvas.git
cd seed-canvas
pnpm install
```

### Common Tasks

```bash
pnpm test          # run unit + integration tests across all workspaces
pnpm lint          # ESLint + Prettier check
pnpm typecheck     # tsc --noEmit across all packages
pnpm build         # produce distributable bundles
pnpm --filter @seed-canvas/cli start -- render --template galaxy --seed hello
```

### Working on a Template

Templates live in `examples/`. Each example exports an `entry(ctx)` function and
a `paramsSchema` object. Use the existing `galaxy` template as the canonical
reference — its comments describe the full contract.

### Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(render): add WebGPU adapter
fix(core): keep fork() deterministic across platforms
docs(readme): clarify share URL format
```

## Pull Request Checklist

- [ ] Tests pass locally (`pnpm test`).
- [ ] Lint passes (`pnpm lint`).
- [ ] Typecheck passes (`pnpm typecheck`).
- [ ] New code is covered by tests where reasonable.
- [ ] Public APIs have JSDoc comments.
- [ ] CHANGELOG.md entry added under "Unreleased".
- [ ] PR description references any related issue.

## Release Process

Releases are automated via `release-please`:

1. Merge feature PRs into `main`.
2. release-please opens a release PR that bumps the version and updates CHANGELOG.
3. Merging the release PR tags the commit and publishes to npm, PyPI, and
   Docker Hub.

## License

By contributing, you agree that your contributions will be licensed under the
project's [MIT License](./LICENSE).