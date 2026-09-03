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

Choose one of the three options

### 1. Installing via crates.io (recommended)

Float is available as a binary crate on crates.io under the name `float-mux` (`float` was already taken, sadly...)

Install it with cargo and run it

```bash
cargo install float-mux
float-mux
```

### 2. Downloading release from GitHub

You can find the binaries for every version tag in the ![releases](https://github.com/Henktorius/float/releases/latest) section

### 3. Building from source

Clone the repository and build with Cargo

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

#### Linux console (no X11)

On a bare virtual console (`TERM=linux`) the terminal emits no mouse escape
sequences, so Float talks to the [`gpm`](https://linux.die.net/man/8/gpm) daemon
directly. Start `gpm` (for example `gpm -m /dev/input/mice -t imps2`) before
launching Float. `libgpm.so` is loaded at runtime; if it or the daemon is
absent, Float runs normally without mouse support. Any other terminal keeps
using crossterm mouse capture unchanged. On the console Float also draws its
own pointer, since the console has none once gpm hands the mouse over.

Inside `screen` or `tmux` on the console it still works: Float connects to gpm
for the active virtual console (read from `/sys/class/tty/tty0/active`) unless a
GUI session is present. Set `FLOAT_GPM_VC=<n>` to force a specific console,
`FLOAT_GPM_VC=auto`, or `FLOAT_GPM_VC=off` to disable.

#### Mouse in child programs

Mouse-aware programs run in a window (`mc`, `vim`, `htop`, `less`, …) receive
the mouse when they enable xterm mouse reporting: clicks on the border and
title bar still move and resize the window, clicks inside go to the program.
Children whose `TERM` would be `linux`, `screen`, or `tmux` are started with
`TERM=xterm-256color`. Disable passthrough with `mouse_passthrough = false`.

## Configuration

Float reads `~/.config/float/config.toml`. Check out the `config.example.toml` file in the repository.
