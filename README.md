# vrm2sl

VRM to SecondLife Avatar bone converter.

## Usage

```bash
cargo run -- input.vrm output.gdb
```

or after install/build:

```bash
vrm2sl input.vrm output.gdb
```

### Analyze only (no export)

```bash
vrm2sl input.vrm output.gdb --analyze-only --report report.json
```

### Main options

- `--target-height <cm>`: target avatar height (default `200`)
- `--manual-scale <n>`: extra scale multiplier (default `1.0`)
- `--resize on|off`: texture auto-resize policy (default `on`)
- `--resize-method bilinear|nearest|bicubic|gaussian|lanczos3`
- `--load-settings <project.json>`: load project settings
- `--save-settings <project.json>`: save current project settings
- `--report <report.json>`: save analysis report JSON

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
- Output `.gdb` (GLB container) generation

## Notes

- Current implementation focuses on Rust core pipeline and CLI.
- Texture auto-resize currently affects validation/estimation and option handling; embedded image payload rewrite is not yet enabled.
- Full hierarchy reconstruction, inverse-bind full regeneration/writeback, and advanced UI/preview workflow are planned next steps.

See [Contributing](CONTRIBUTING.md) for development and coding conventions.

## Testing

- Run tests with `cargo test`.
- Name unit tests in Given/When/Then style for behavior clarity.
- Example pattern: `given_condition_when_action_then_expected_result`.
