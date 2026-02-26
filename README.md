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

## Implemented (core MVP baseline)

- VRM/GLB input loading
- VRoid model validation (non-VRM/unsupported source is rejected)
- Required humanoid bone presence check
- VRM → Second Life bone-name mapping for core humanoid bones
- Uniform avatar scaling toward SL default height (200cm)
- Removal of VRM extension references/extras from output JSON chunk
- Removal of animation and morph target entries
- Output `.gdb` (GLB container) generation

## Notes

- Current implementation focuses on Rust core pipeline and CLI.
- Texture auto-resize options are modeled in API but not yet applied to embedded image payloads.
- Full hierarchy reconstruction, inverse-bind full regeneration/writeback, and advanced UI/preview workflow are planned next steps.

See [Contributing](CONTRIBUTING.md) for development and coding conventions.

## Testing

- Run tests with `cargo test`.
- Name unit tests in Given/When/Then style for behavior clarity.
- Example pattern: `given_condition_when_action_then_expected_result`.
