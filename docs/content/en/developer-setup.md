# Developer Setup

## Prerequisites

- Node.js and pnpm
- Rust toolchain
- Tauri build prerequisites for your OS

## Install

```bash
pnpm install
```

## Run desktop app

```bash
pnpm dev:tauri
```

## Run Rust CLI

```bash
cargo run --manifest-path backend/Cargo.toml --bin vrm2sl -- input.vrm output.glb
```
