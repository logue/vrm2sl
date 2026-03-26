---
description: "Use when creating or reorganizing project structure in vrm2sl; align outer directory and responsibility boundaries with tauri-vuetify-starter style (frontend/backend/docs/scripts split)."
---

Prefer tauri-vuetify-starter style outer structure when adding new areas or reorganizing files.

## Baseline Structure Intent

- `frontend/`: Vue + Vite UI application code.
- `backend/`: Rust + Tauri app core and command surface.
- `scripts/`: build/release/maintenance automation.
- `docs/`: documentation site or user/developer docs.
- `docker/`: containerized build/runtime helpers.

## Placement Rules

- UI components, composables, stores, plugins, and view-level types belong under `frontend/src/`.
- Tauri commands and Rust business logic belong under `backend/src/`.
- Cross-cutting developer automation belongs in `scripts/`.
- New top-level directories require a short rationale in PR/commit notes.

## Change Discipline

- Prefer additive structure changes over disruptive moves.
- If moving files, preserve behavior first and refactor in a follow-up step.
- Keep naming and boundaries consistent with existing `frontend/` and `backend/` split.
