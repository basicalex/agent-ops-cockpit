# Contributing to Agent Ops Cockpit

We welcome contributions!

## Development Setup

1.  **Prerequisites:**
    *   Rust (latest stable)
    *   Bun
    *   Herdr
    *   `yazi`

2.  **Build:**
    ```bash
    cargo build --manifest-path crates/Cargo.toml --workspace
    ```

3.  **Install Locally:**
    ```bash
    ./install.sh
    ```

## Project Structure

*   `crates/`: Rust binaries (`aoc-cli`).
*   `bin/`: Shell scripts and wrappers.
*   `herdr/`: Herdr config baseline.
*   `yazi/`: Yazi config and plugins.
