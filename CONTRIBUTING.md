# Contributing to Agent Ops Cockpit

We welcome contributions!

## Development Setup

1.  **Prerequisites:**
    *   Rust (latest stable)
    *   `zellij` >= 0.43.1
    *   `yazi`

2.  **Build:**
    ```bash
    cargo build --workspace
    ```

3.  **Install Locally:**
    ```bash
    ./install.sh
    ```

## Project Structure

*   `crates/`: Rust binaries (`aoc-cli`, `aoc-core`).
*   `plugins/`: Zellij WASM plugins (`taskmaster`).
*   `bin/`: Shell scripts and wrappers.
*   `zellij/`: Layout templates.
