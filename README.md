# vrm2sl

![screenshot](./images/ss1.png)

VRM to SecondLife Avatar bone converter.

## Usage

```bash
cargo run --manifest-path backend/Cargo.toml --bin vrm2sl -- input.vrm output.glb
```

or after install/build:

```bash
cargo install --path backend --bin vrm2sl
vrm2sl input.vrm output.glb
```

### Analyze only (no export)

```bash
vrm2sl input.vrm output.glb --analyze-only --report report.json
```

When analysis or conversion completes, a desktop notification is sent (on macOS, via `osascript`).

### Main options

- `--target-height <cm>`: target avatar height (default `200`)
- `--manual-scale <n>`: extra scale multiplier (default `1.0`)
- `--resize on|off`: texture auto-resize policy (default `on`)
- `--resize-method bilinear|nearest|bicubic|gaussian|lanczos3`
- `--load-settings <project.json>`: load project settings
- `--save-settings <project.json>`: save current project settings
- `--report <report.json>`: save analysis report JSON

## Tauri IPC integration layer

The Rust side exposes IPC-friendly functions designed for easy integration with Tauri `invoke`.

- `ipc::analyze_vrm_ipc`
- `ipc::convert_vrm_to_gdb_ipc`
- `ipc::save_project_settings_ipc`
- `ipc::load_project_settings_ipc`

Each function accepts `String` paths and serializable request structs,
with input/output shapes that can be called directly from the UI layer.

## Tauri + Vuetify app (integrated)

`tauri-vuetify-starter` is integrated into this repository.

- Frontend: `frontend/`
- Tauri backend: `backend/`
- Rust core implementation: `backend/src/`

### Desktop app run

```bash
pnpm install
pnpm dev:tauri
```

From `MainContent` in the UI, you can:

- Select a VRM file
- Run analysis (bones/vertices/textures/upload cost estimate)
- Save/load settings JSON
- Export `.glb`

Desktop notifications are sent when analysis/conversion completes.

## Implemented (core MVP baseline)

- VRM/GLB input loading
- VRoid model validation (non-VRM/unsupported source is rejected)
- Required humanoid bone presence check
- Required humanoid parent-child hierarchy validation
- VRM → Second Life bone-name mapping for core humanoid bones
- Uniform avatar scaling toward SL default height (200cm)
- Mesh statistics (vertices/polygons) and 65535 vertex-limit diagnostics
- Texture resolution diagnostics and upload-fee estimate (before/after resize)
- Removal of VRM extension references/extras from output JSON chunk
- Removal of animation and morph target entries
- Project settings JSON load/save API
- Output `.glb` (GLB container) generation

## Notes

- Current implementation focuses on Rust core pipeline and CLI.
- Texture auto-resize currently affects validation/estimation and option handling; embedded image payload rewrite is not yet enabled.
- Full hierarchy reconstruction, inverse-bind full regeneration/writeback, and advanced UI/preview workflow are planned next steps.

## Stability Notes (Eyes/Face)

Recent fixes for face/eye instability (cross-eye, missing iris, flicker) are now part of the default pipeline.

- Conversion side:
	- `skin.skeleton` prefers `mPelvis` when available.
	- Face skin keeps `mHead/mEyeLeft/mEyeRight` joints.
	- Hair-like tiny secondary skins may be simplified to `mHead` for stability.
	- Bind-pose correction excludes tiny face eye skins to avoid eye drift.
- Preview side (`frontend/src/components/VrmPreview.vue`):
	- Eye materials are handled separately from lash/brow materials.
	- Iris/highlight use anti-z-fighting settings (`polygonOffset`) to reduce depth sorting artifacts.
	- Upper-body BVH retargeting is enabled (wrists are still filtered to reduce hand collapse).

If eye placement looks wrong, validate in this order:

1. Regenerate output with current backend (`cargo run --manifest-path backend/Cargo.toml --bin vrm2sl -- <input.vrm> <output.glb>`).
2. Confirm skin topology with `python3 vrm/inspect_output.py`.
3. Check face skin influence with `python3 vrm/inspect_skin0_weights.py`.
4. Compare eye-weighted centers with `python3 vrm/inspect_eye_vertex_positions.py vrm/output.glb`.

## Animation Attribution

Contains animation data © Linden Research, Inc.  
Licensed under CC BY 3.0  
https://creativecommons.org/licenses/by/3.0/

Modified for use in this tool.

Frontend-side asset note: see [frontend/README.md](frontend/README.md).

See [Contributing](CONTRIBUTING.md) for development and coding conventions.

## Testing

- Run tests with `cargo test`.
- Name unit tests in Given/When/Then style for behavior clarity.
- Example pattern: `given_condition_when_action_then_expected_result`.
