# Finger Debug Summary

## Scope

- Focus only on non-thumb finger behavior in Second Life/Bento hand animation.
- Wrist motion itself looked normal, so investigation stayed limited to finger bones and finger skinning.

## Confirmed Findings

- The best visual result in this chat was `AvatarSample_A_fixed7.glb`.
- Later rollback outputs `fixed14` and `fixed15` matched the `fixed7` finger baseline for the inspected hand joints.
- Source VRM versus `fixed7` comparisons did not show a meaningful loss of finger fan-out when evaluated in the hand-local frame.
- Experiments that modified non-thumb finger bone transforms directly did not improve the in-world result.
- Experimental spread/splay/fan bone adjustments made the result worse and were removed.
- A proximal-only normalization experiment for non-thumb fingers did not help and was reverted.

## What Helped

- `cleanup_cross_finger_family_weights` in `backend/src/convert/skinning.rs` reduced cross-finger contamination.
- The cleanup keeps only the dominant non-thumb finger family, removes thumb influence from non-thumb vertices, and removes wrist influence from those vertices.
- Verified metrics after cleanup on the sample model:
  - `non_thumb_vertices = 4696`
  - `with_thumb = 0`
  - `with_wrist = 0`

## What Did Not Help

- Translation-only remaps on finger bones caused visible stretching.
- Extra non-thumb proximal spread multipliers did not solve the inward look.
- Added non-thumb proximal splay rotations did not solve the inward look.
- Added intermediate/distal fan-out translation offsets made the result worse.
- Normalizing only non-thumb proximal rotations did not produce a useful change.

## Diagnostic Output Added

- Conversion diagnostics now include `finger_representative_vertices` in the output diagnostic JSON.
- This records one representative vertex per side/finger/segment:
  - 2 sides
  - 4 fingers
  - 3 segments
  - total: 24 entries
- For `AvatarSample_A_fixed17.diagnostic.json`, all 24 representative vertices were single-joint dominant with:
  - `dominant_joint_weight = 1.0`
  - `family_weight = 1.0`

## Current Interpretation

- The remaining visual problem is unlikely to be explained by simple finger-bone rest transform edits alone.
- The next useful direction is finger-only deformation tracking at the mesh level during hand poses, using representative vertices and family-isolated vertices.
- Any future investigation should stay focused on finger deformation and avoid broadening to wrist or whole-hand motion unless new evidence appears.

## Relevant Code Paths

- `backend/src/convert/skinning.rs`
- `backend/src/convert/diagnostic.rs`
- `backend/src/convert/skeleton.rs`
- `backend/src/convert/mod.rs`

## Session End State

- Finger diagnostic support was added and pushed in commit `102fcfc`.
- The diagnostic note from this chat is intentionally stored in the repository under `vrm/FINGER_DEBUG_SUMMARY.md` for later continuation.
