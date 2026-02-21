# Contributing

Thanks for contributing to vrm2sl.

## Development Setup

- Install Rust stable toolchain.
- Run tests with `cargo test`.

## Coding Guidelines

- Keep changes minimal and focused on the requested behavior.
- Write documentation comments in English for public structs and functions.
- Use clear, descriptive names for types and functions.
- Prefer `thiserror` for public error enums and provide readable error messages.

## Rustdoc Template

Use this template for public functions when applicable:

```rust
/// Short summary.
///
/// # Arguments
///
/// * `arg_name` - Description.
///
/// # Returns
///
/// Description of return value.
///
/// # Errors
///
/// Description of possible errors.
```

Notes:

- Skip `# Errors` for infallible functions.
- Keep summaries concise and behavior-focused.
- Prefer domain terms used in this project (bone, node, bind matrix, T-pose).

## Struct Doc Template

Use this template for public structs and their fields:

```rust
/// Short summary of what this struct represents.
pub struct ExampleStruct {
	/// Meaning of this field in the conversion pipeline.
	pub important_field: String,
}
```

Notes:

- Add a struct-level summary first, then document each public field.
- Keep field comments practical and data-flow oriented.
- Prefer wording that matches runtime usage (input, output, parsed, corrected).

## Enum Doc Template

Use this template for public enums and their variants:

```rust
/// Short summary of what this enum represents.
pub enum ExampleEnum {
	/// Description of this variant.
	Simple,
	/// Description of this variant and when it is returned.
	Detailed {
		/// Meaning of this field.
		reason: String,
	},
}
```

Notes:

- Add an enum-level summary first, then document each variant.
- Document variant fields when using struct-like variants.
- For error enums, describe when each variant is produced.

## Test Guidelines

- Add unit tests for new logic when practical.
- Use Given/When/Then style naming for test functions.
- Preferred pattern: `given_condition_when_action_then_expected_result`.

## Pull Request Checklist

- Ensure `cargo test` passes.
- Confirm public API changes are documented.
- Update `README.md` if user-facing behavior changes.
