# prr — keyboard-driven TUI for GitHub PR review

A terminal UI for reviewing GitHub pull requests without leaving your editor workflow. Browse open PRs, read diffs and comments, and check out branches — all from the keyboard.

## Features

- Browse open PRs for the current repository
- Toggle between your PRs and all PRs
- Inline diff and comment views
- Difftastic-powered syntax-aware diffs
- Theme picker with multiple built-in themes
- Check out PR branches directly from the TUI
- Open any PR in the browser
- **Stacked PR support** — detects stacked PRs automatically and groups them in the list; navigate between stack members with `[` / `]`

## Requirements

- [Rust](https://rustup.rs/) (stable)
- [gh](https://cli.github.com/) — GitHub CLI, authenticated (`gh auth login`)
- A terminal that supports 256 colors

## Installation

```bash
cargo install --path .
```

This installs the `prr` binary.

## Usage

Run `prr` inside any Git repository that has a GitHub remote:

```bash
prr
```

The tool auto-detects the repository from your working directory.

## Keybindings

### PR List

| Key     | Action                  |
|---------|-------------------------|
| `j / k` | Navigate up / down      |
| `Enter` | Open PR detail          |
| `a`     | Toggle mine / all PRs   |
| `r`     | Refresh PR list         |
| `o`     | Open PR in browser      |
| `q`     | Quit                    |

### PR Detail

| Key       | Action                              |
|-----------|-------------------------------------|
| `Tab`     | Switch Diff ↔ Comments ↔ Difftastic |
| `j / k`   | Scroll                              |
| `n / N`   | Next / previous changed file        |
| `[ / ]`   | Jump to PR below / above in stack   |
| `c`       | Checkout PR branch                  |
| `o`       | Open PR in browser                  |
| `Esc / q` | Back to list                        |

### Stacked PRs

`prr` automatically detects stacked PRs using base/head branch matching — no
stacking tool required. When PR B's base branch equals PR A's head branch, B is
considered stacked on A.

In the **PR list**, stacked PRs are grouped contiguously with tree connectors
(`┬`, `├`, `└`) and a stack-size badge (e.g. `≡3`) on the bottom PR.

In the **PR detail** header, a Stack line shows all members of the stack with
the current PR highlighted:

```
Stack:  [#121 auth-layer] → [#122 api-routes ★] → [#123 frontend]
```

Use `[` to jump to the PR below (closer to trunk) and `]` to jump to the PR
above (further from trunk).

### Global

| Key | Action            |
|-----|-------------------|
| `T` | Open theme picker |
| `?` | Toggle help       |

## License

MIT
