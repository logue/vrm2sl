# 開発セットアップ

## 前提

- Node.js と pnpm
- Rust toolchain
- OS に応じた Tauri ビルド要件

## 依存関係のインストール

```bash
pnpm install
```

## デスクトップアプリ起動

```bash
pnpm dev:tauri
```

## Rust CLI 実行

```bash
cargo run --manifest-path backend/Cargo.toml --bin vrm2sl -- input.vrm output.glb
```
