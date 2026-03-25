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
| `c`       | Checkout PR branch                  |
| `o`       | Open PR in browser                  |
| `Esc / q` | Back to list                        |

### Global

| Key | Action            |
|-----|-------------------|
| `T` | Open theme picker |
| `?` | Toggle help       |

## License

MIT
