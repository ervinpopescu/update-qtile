# update-qtile

## Project Overview

`update-qtile` is a single-binary Rust CLI tool for Arch Linux that automates building and installing [qtile-git](https://aur.archlinux.org/packages/qtile-git) from the AUR with custom source overrides. It clones the AUR PKGBUILD, patches it to point at a specified fork/branch/commit/tag/PR, builds with `makepkg`, and installs with `pacman`. Optionally restarts qtile via IPC (`qtile-cmd-client`).

## Build & Run

```bash
cargo build            # debug build
cargo build --release  # release build
cargo run -- --help    # show CLI usage
cargo clippy           # lint
```

No test suite exists; the project has no `tests/` directory or `#[cfg(test)]` blocks.

## Architecture

Single-file application (`src/main.rs`) with one struct `UpdateQtile` driving the pipeline:

1. **`get_source()`** — resolves CLI args (`--fork`, `--path`, `--branch`, `--commit`, `--tag`, `--pull`) into a `Source` (git URL + fragment).
2. **`remove_repo()`** — clears the cached AUR clone at `$XDG_CACHE_HOME/yay/qtile-git`.
3. **`clone_repo()`** — clones the AUR repo, then calls `modify_pkgbuild()`.
4. **`modify_pkgbuild()`** — regex-patches the PKGBUILD: sets custom `source=`, adds `groups=('modified')`, and for PRs fetches/checks out the PR branch.
5. **`install()`** — runs `makepkg -rsc --nocheck`, removes the old package, installs the new `.tar.zst` via `sudo pacman -U`, optionally restarts qtile via IPC.

CLI is built with `clap` (derive). Logging uses `simple_logger` + `log`. External commands run via the `subprocess` crate.

## Key Dependencies

- `git2` — git clone operations
- `qtile-cmd-client` — IPC restart (from `github.com/ervinpopescu/qtile-cmd-client`)
- `subprocess` — shell-out to `makepkg`, `pacman`, `sudo`
- `text_io` — interactive Y/n prompts
