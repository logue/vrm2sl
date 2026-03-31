# Architecture

vrm2sl follows the split style from tauri-vuetify-starter.

- frontend: Vue + Vite UI
- backend: Rust + Tauri
- scripts: automation scripts
- docker: reproducible build helpers
- docs: Nuxt multilingual documentation site

## Runtime flow

1. Select VRM in frontend
2. Invoke Tauri commands
3. Run analysis/conversion in backend
4. Reflect results in UI
