# アーキテクチャ

vrm2sl は tauri-vuetify-starter の分割方針に合わせています。

- frontend: Vue + Vite UI
- backend: Rust + Tauri
- scripts: 自動化スクリプト
- docker: 再現可能ビルド補助
- docs: Nuxt 多言語ドキュメントサイト

## ランタイムフロー

1. フロントエンドで VRM を選択
2. Tauri コマンドを呼び出し
3. バックエンドで解析/変換
4. 結果を UI へ反映
