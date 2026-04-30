# AGENT.md

This file defines repository-wide common rules for AI coding agents.

---

## Scope and Priority

Use rules in this order:

1. Directory-specific AGENT.md (frontend/, docs/, backend/) when present
2. Instructions under .github/instructions/\* and .github/copilot-instructions.md
3. This root AGENT.md

When rules conflict, prefer the rule with narrower scope.

---

## Repository Baseline

- Monorepo: frontend (Vue/Vite), backend (Rust/Tauri), docs (Nuxt)
- Package manager: pnpm only
- Keep this repository template-generic; do not add product-specific logic
- .env is the source of truth for app metadata/version; do not manually desync version fields

---

## Downstream App Specification (Fill This Section)

This repository is a template. When it is used for a concrete app, fill in this section first.

### App Profile

- App name: vrm2sl (VRM to Second Life Converter)
- Domain/business context: 3D avatar format conversion — VRM 1.0 (glTF extension) to Second Life Bento-compatible GLB
- Target users: Second Life residents who own VRM avatars and want to import them into Second Life
- Supported platforms: Windows, macOS, Linux
- Release constraints: Offline-only; no external network calls during conversion

### Core Features

- Feature 1: VRM 1.0 file analysis — validates humanoid bone mapping, detects missing bones, estimates avatar height, and reports texture info
- Feature 2: VRM to GLB conversion — remaps skeleton to SL Bento rig, corrects bind-pose rotations (finger Y-axis zeroing), rescales to target height, and optionally downscales textures
- Feature 3: Real-time 3D preview — renders the loaded VRM/GLB using Three.js with orbit camera, idle animation, and BVH retargeting support

### Functional Requirements

- FR-1: Accept VRM 1.0 input files via file picker dialog or drag-and-drop
- FR-2: Produce a GLB output compatible with Second Life's Bento skeleton importer
- FR-3: Report conversion metrics (scale factor, texture counts, validation issues) to the user

### Non-Functional Requirements

- Performance: Analysis and conversion of typical VRM files (< 100 MB) must complete within a reasonable time on a modern desktop
- Security: File I/O is sandboxed via Tauri capabilities; no arbitrary shell execution
- Accessibility: Vuetify-based UI; follow standard keyboard navigation patterns
- Localization: UI text must be present in en, ja, fr, ko, zhHans, zhHant
- Observability/logging: Rust backend emits structured log events via `send_log_with_handle`; frontend displays them in the log panel

### Out of Scope

- VRM 0.x support (tool explicitly rejects VRM 0.x files)
- Upload to Second Life servers
- Batch/folder conversion

### Project-Specific Decisions

- Naming/domain terms: "SL" = Second Life; "Bento" = SL's extended skeleton with finger/face bones; "VRM" = Virtual Reality Model (glTF extension by VRM Consortium)
- API/command naming conventions: Tauri commands use snake_case with `_command` suffix (e.g. `analyze_vrm_command`, `convert_vrm_command`)
- Storage and data retention policy: Project settings are persisted as JSON by the user on explicit save; no auto-save
- Error handling policy: Tauri commands return `Result<T, String>`; frontend shows errors via `useNotification`

Rule: Before implementing app-specific features, check this section and align implementation decisions with it.

---

## Documentation Comment Rule (Mandatory)

For all generated code, documentation comments in English are mandatory.

### TypeScript / JavaScript

Add JSDoc-compliant comments for generated:

- functions (including exported arrow functions)
- constants
- classes

Use tags when applicable:

- @param
- @returns
- @throws
- @example

### Rust

Add Rustdoc comments for generated:

- functions
- constants
- types (struct, enum, trait)

Use sections when applicable:

- # Arguments
- # Returns
- # Errors
- # Panics
- # Examples

This applies to newly created symbols and symbols modified during refactoring.

---

## Directory Guides

- frontend/: frontend/AGENT.md
- docs/: docs/AGENT.md
- backend/: backend/AGENT.md
