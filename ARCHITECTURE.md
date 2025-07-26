# Project Structure

## Overview
Katori is organized as a Rust workspace with separate packages for different concerns:

```
katori/
├── Cargo.toml          # Workspace root configuration
├── src/main.rs         # Main application entry point
├── katori-gui/         # GUI package using egui
│   ├── Cargo.toml
│   └── src/lib.rs
├── gdbadapter/         # GDB communication package
│   ├── Cargo.toml
│   └── src/lib.rs
└── README.md
```

## Packages

### katori (root)
- Main binary that orchestrates the GUI and GDB adapter
- Minimal entry point that initializes both components

### katori-gui
- GUI implementation using egui framework
- Creates standalone desktop application (no browser required)
- Handles user interface and user interactions

### gdbadapter
- Handles communication with GDB using GDB/MI protocol
- Can be extracted as a standalone crate later
- Provides high-level API for debugging operations

## Running the Application

```bash
cargo run
```

This will compile and launch the standalone desktop application.

## Building

```bash
cargo build --release
```

Creates optimized executable in `target/release/katori.exe`

## Features

- ✅ Standalone desktop application (no browser/web dependencies)
- ✅ Modern Rust GUI using egui
- ✅ Modular architecture for easy extraction of components
- 🚧 GDB integration (planned)
- 🚧 Debugging interface (planned)
