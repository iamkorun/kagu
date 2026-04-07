<p align="center">
  <h1 align="center">🐦 kagu</h1>
  <p align="center">Zero-config conventional commit auditor. Single binary, no Node required.</p>
</p>

<p align="center">
  <a href="https://github.com/iamkorun/kagu/actions/workflows/ci.yml"><img src="https://github.com/iamkorun/kagu/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/kagu"><img src="https://img.shields.io/crates/v/kagu.svg" alt="crates.io"></a>
  <a href="https://github.com/iamkorun/kagu/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://github.com/iamkorun/kagu/stargazers"><img src="https://img.shields.io/github/stars/iamkorun/kagu?style=social" alt="Stars"></a>
  <a href="https://buymeacoffee.com/iamkorun"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"></a>
</p>

---

## The Problem

Conventional commits are easy to forget and hard to enforce consistently. Existing tools like `commitlint` need Node.js, a config file, and a husky setup just to get started. `cocogitto` is a full release-management suite — way more than you need just to audit commits. `gitlint` is Python-only and config-heavy.

You just want to know: _are my commits spec-compliant?_ Without installing a JavaScript runtime.

## The Solution

**kagu** is a single Rust binary. `cargo install kagu` and you're done. No config file. No runtime. No plugins. Point it at any git repo and it immediately tells you which commits break the [Conventional Commits](https://www.conventionalcommits.org/) spec — with exit codes, JSON output, per-author stats, and a one-command git hook install.

Named after the [kagu](https://en.wikipedia.org/wiki/Kagu) — a rare, crested bird from New Caledonia with a piercing call that alerts you when something's wrong.

## Demo

![demo](docs/demo.gif)

```
$ kagu scan

kagu scan

  ✗ a1b2c3d  WIP stuff happening
      error [format] subject does not match `<type>(<scope>)?!?: <description>`
  ! d4e5f67  feat: add database connection.
      warn  [punctuation] description should not end with `.`
  ✗ 7890abc  release the thing
      error [format] subject does not match `<type>(<scope>)?!?: <description>`

summary
  total: 24  clean: 21  warnings: 1  errors: 2  skipped: 1
  score: 87/100

types
  chore       4
  feat        9
  fix         5
  refactor    3
```

## Quick Start

```sh
cargo install kagu
cd your-project/
kagu scan
```

## Installation

### From crates.io

```sh
cargo install kagu
```

### From source

```sh
git clone https://github.com/iamkorun/kagu.git
cd kagu
cargo install --path .
```

### From releases

Pre-built binaries for Linux, macOS, and Windows are available on the [Releases](https://github.com/iamkorun/kagu/releases) page.

## Usage

### Scan full git history

```sh
kagu scan
```

```
kagu scan

  ✗ a1b2c3d  WIP stuff happening
      error [format] subject does not match `<type>(<scope>)?!?: <description>`

summary
  total: 12  clean: 11  warnings: 0  errors: 1  skipped: 0
  score: 91/100
```

### CI-friendly: only new commits on a branch

```sh
kagu scan --since main
```

Use in GitHub Actions to catch violations before they merge:

```yaml
- name: Audit commits
  run: kagu scan --since origin/main --quiet
```

Exit code `1` if any violations are found — CI fails automatically.

### Machine-readable JSON output

```sh
kagu scan --json
```

```json
{
  "commits": [
    {
      "sha": "a1b2c3d4e5f6...",
      "author": "dev@example.com",
      "subject": "WIP stuff happening",
      "status": "error",
      "violations": [
        {
          "code": "format",
          "message": "subject does not match `<type>(<scope>)?!?: <description>`",
          "severity": "error"
        }
      ]
    }
  ],
  "summary": {
    "total": 12,
    "clean": 11,
    "warnings": 0,
    "errors": 1,
    "skipped": 0,
    "score": 91,
    "by_type": { "feat": 5, "fix": 3, "chore": 3 }
  },
  "authors": null
}
```

### Per-author breakdown

```sh
kagu scan --authors
```

```
authors
  author                     total  clean  errors  score
  alice@example.com             14     14       0  100/100
  bob@example.com               8      6       2   75/100
  carol@example.com             2      1       1   50/100
```

### Strict mode (require scope on every commit)

```sh
kagu scan --strict
```

`feat: add thing` → error: `[scope] strict mode requires (scope) on every commit`
`feat(cli): add thing` → clean ✓

### Install a commit-msg hook

Never merge a bad commit again:

```sh
kagu hook install
```

```
kagu: installed commit-msg hook at .git/hooks/commit-msg
```

Won't clobber an existing hook. To remove:

```sh
kagu hook uninstall
```

### Lint a single commit message

```sh
echo "bad commit message" | kagu lint /dev/stdin
# or
kagu lint .git/COMMIT_EDITMSG
```

```
kagu: commit message rejected
  error [format] subject does not match `<type>(<scope>)?!?: <description>`
```

## Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--quiet` | `-q` | Suppress output, only set exit code |
| `--verbose` | `-v` | Print every commit, not just violations |

### `scan` flags

| Flag | Description |
|------|-------------|
| `--path <PATH>` | Repository path (default: `.`) |
| `--since <REF>` | Only scan commits in `<ref>..HEAD` |
| `--authors` | Include per-author breakdown |
| `--json` | Output as JSON |
| `--strict` | Require `(scope)` on every commit |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All commits are spec-compliant |
| `1` | One or more violations found |
| `2` | System error (invalid path, git not found) |

## Configuration

There is none.

kagu validates against the [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) spec with these allowed types out of the box:

`feat` `fix` `docs` `style` `refactor` `perf` `test` `build` `ci` `chore` `revert`

Merge commits and "Initial commit" are skipped automatically.

## Features

- **Zero config** — no config file, no setup wizard, no plugins
- **Single binary** — `cargo install kagu`, done; no Node.js, no Python
- **CI-ready** — `--since main` scans only branch commits; exit codes play nice with CI
- **JSON output** — pipe into scripts, dashboards, or your own tooling
- **Per-author stats** — find who needs a refresher on conventional commits
- **Strict mode** — enforce `(scope)` on every commit when your team requires it
- **Git hook** — `kagu hook install` adds a `commit-msg` hook in one command
- **Smart skipping** — merge commits and initial commits are ignored automatically
- **Tiny** — ~644K stripped binary, only 4 runtime dependencies

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change.

```sh
git clone https://github.com/iamkorun/kagu.git
cd kagu
cargo test
```

## License

[MIT](LICENSE)

---

## Star History

<a href="https://star-history.com/#iamkorun/kagu&Date">
  <img src="https://api.star-history.com/svg?repos=iamkorun/kagu&type=Date" alt="Star History Chart" width="600">
</a>

---

<p align="center">
  <a href="https://buymeacoffee.com/iamkorun"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me a Coffee" width="200"></a>
</p>
