# qup (Qtile UPdater)

`qup` is a Rust CLI tool designed to automate the process of building and installing [Qtile](https://qtile.org/) from source. It supports various installation backends and allows users to easily test custom forks, branches, tags, commits, or pull requests.

## Features

- **Multi-backend support**: Works on Arch Linux, Debian/Ubuntu, and via the `uv` package manager.
- **Source flexibility**: Install from any GitHub fork, specific branches, tags, commits, or even pull requests.
- **Automatic dependency management**: Handles native dependencies for supported distributions.
- **Qtile integration**: Optionally restarts Qtile via IPC after a successful installation.
- **Auto-detection**: Automatically detects your distribution to choose the appropriate installation method.

## Supported Backends

- **Arch Linux**: Clones the `qtile-git` AUR package, patches the `PKGBUILD` with your custom source, and installs it using `makepkg` and `pacman`.
- **Debian/Ubuntu**: Installs necessary native dependencies via `apt` and then uses the `uv` backend for installation.
- **uv**: Uses [astral-sh/uv](https://github.com/astral-sh/uv) to install Qtile as a tool (`uv tool install`), providing a clean and isolated installation.

## Installation

### Prerequisites

- **Rust**: You'll need the Rust toolchain installed.
- **uv** (optional, but recommended): Required for the `uv` and `debian` backends.
- **Qtile**: The tool expects Qtile to be running if you use the `--restart` flag.

### Building from source

```bash
git clone https://github.com/ervinpopescu/update-qtile.git
cd update-qtile
cargo build --release
```

The binary will be available at `target/release/qup`.

## Usage

```bash
qup [OPTIONS]
```

### Examples

#### Install from a specific fork and branch
```bash
qup --fork myuser --branch experimental-feature
```

#### Install from a Pull Request
```bash
qup --pull 1234
```

#### Install a specific commit
```bash
qup --commit abcdef123456
```

#### Force a specific installation method
```bash
qup --method uv
```

#### Install and restart Qtile automatically
```bash
qup --restart
```

### Options

- `-f, --fork <FORK>`: GitHub fork to use (default: "qtile").
- `-P, --path <PATH>`: Local path to a Qtile repository.
- `-c, --commit <COMMIT>`: Specific git commit hash.
- `-b, --branch <BRANCH>`: Specific git branch.
- `-t, --tag <TAG>`: Specific git tag.
- `-p, --pull <PULL>`: GitHub Pull Request number.
- `-r, --restart`: Restart Qtile via IPC after installation.
- `-m, --method <METHOD>`: Installation method (`arch`, `uv`, `debian`).
- `-V, --version`: Print version.
- `-h, --help`: Print help.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
