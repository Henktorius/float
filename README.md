<div align="center">

# Float

**Floating window multiplexer for your terminal, written in Rust.**

<img alt="demo" src="https://github.com/user-attachments/assets/eca7ed1d-8ad7-4ccd-a8c9-6a2297145586"/>

</div>

---

## Features

- Freely positioned, overlapping terminal windows
- Keyboard-driven window management
- Mouse support: drag title bars to move, drag edges to resize
- Configurable colors and key bindings via TOML

## Requirements

- A real terminal emulator (Float uses raw mode and the alternate screen;
  integrated IDE terminals are not supported yet)
- Linux (uses Unix PTY APIs and `/proc`)
- Rust toolchain

## Getting Started

### Option a: Installing via crates.io

Float is available as a binary crate on crates.io under the name `float-mux` (`float` was already taken, sadly...)

Install it with cargo and run it.

```bash
cargo install float-mux
float-mux
```

### Option b: Building from source

Clone the repository and build with Cargo.

```bash
git clone https://github.com/henktorius/float
cd float
cargo build --release
```

The binary will be at `target/release/float-mux`.

### Keyboard shortcuts

| Action             | Default       |
|--------------------|---------------|
| New window         | `Alt+c`       |
| Focus next window  | `Alt+n`       |
| Focus previous     | `Alt+p`       |
| Focus by number    | `Alt+1`–`9`   |
| Move window left   | `Alt+h` / `Alt+←` |
| Move window down   | `Alt+j` / `Alt+↓` |
| Move window up     | `Alt+k` / `Alt+↑` |
| Move window right  | `Alt+l` / `Alt+→` |
| Resize left edge   | `Alt+H` / `Alt+Shift+←` |
| Resize bottom edge | `Alt+J` / `Alt+Shift+↓` |
| Resize top edge    | `Alt+K` / `Alt+Shift+↑` |
| Resize right edge  | `Alt+L` / `Alt+Shift+→` |
| Close window       | `Alt+x`       |
| Quit Float         | `Alt+q`       |

### Mouse

- **Move**: drag the title bar of any window
- **Resize**: drag the left, right, bottom, or bottom-corner edges
- **Focus**: click on any window

## Configuration

Float reads `~/.config/float/config.toml`. Check out the `config.example.toml` file in the repository.
