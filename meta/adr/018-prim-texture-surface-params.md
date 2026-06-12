# ADR-018: Prim Texture Surface Parameters

---
**Metadata:**
- **ID**: ADR-018
- **Status**: Proposed
- **Date**: 2026-06-12
- **Tags**: [prims, textures, ui, protocol, persistence, rendering]
- **Decision Type**: architecture_pattern
- **Aligns with**: N/A — extends prim authoring under ADR-017
- **Doc resolution**: R1
- **Related**: [ADR-017]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: The prim edit dialog's Texture tab only exposed base color and a texture
pick. Authoring parity with Second Life's Texture tab needs per-prim surface controls:
transparency, glow, full-bright, repeats per face (+ flip), texture rotation, repeats
per meter, and offset.

**Requirements**:
- Edit, live-preview, persist, and (when online) replicate these values per prim.
- Map cleanly onto Bevy 0.16 `StandardMaterial` without a custom shader.
- Stay backward-tolerant for stored prims that predate the columns.

## Decision

Add a single shared `PrimSurface` value type (`vibe_core`) carrying `alpha`, `glow`,
`full_bright`, `repeat_u/v`, `flip_u/v`, `rotation`, `offset_u/v`, threaded through the
`Prim` component, `PrimDto`, the `UpdatePrim` message, both SQLite schemas, and the
edit dialog.

**Approach**:
- **Rendering** (`StandardMaterial`): `alpha` → `base_color` alpha + `AlphaMode::Blend`
  when < 1; `glow` → `emissive`; `full_bright` → `unlit`; repeats/flip/rotation/offset →
  a single `uv_transform: Affine2` (`from_scale_angle_translation`). Prim texture images
  use a Repeat-address sampler so repeats tile.
- **Repeats Per Meter** is a transient editor helper: an Apply button derives
  `repeat_u/v` from the object size; it is not stored or wired (per-face repeats are the
  source of truth).
- **Protocol**: `PROTOCOL_VERSION` 7 → 8 (postcard is positional, so an added field is a
  wire break). `PrimSurface` carries `#[serde(default)]` for forgiving JSON/decode.
- **Persistence**: client `ALTER TABLE` migrations + server migration `V3`, which also
  backfills the previously server-only-defaulted geometry columns (path-cut/hollow/warp),
  letting `load_world`/`select_prim_by_id` read real values.

## Rationale

**Primary Reasoning**:
1. One grouped value type keeps ~20 construction/threading sites to a single field each.
2. `StandardMaterial` already exposes every needed property in 0.16 — no shader work.
3. Following the existing warp-param pattern minimizes reviewer surprise.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|---|---|---|---|
| Ten flat fields on every struct | explicit | ~200 lines of threading churn | grouped struct is far smaller |
| Custom material/shader for UV + glow | full control | new pipeline, more maintenance | `StandardMaterial` suffices |
| Carry full `PrimDto` in `UpdatePrim` | future-proof | large test/handler churn | `surface` + `geometry` value types are enough |

## Notes / Limitations

- Texture rotation and repeats pivot about the texture center (0.5, 0.5).
- `UpdatePrim` carries both `PrimSurface` and `PrimGeometry`; the server persists all of
  them, so surface and geometry survive a reconnect / fresh `WorldSnapshot`.
- **Online prim authoring is now wired end-to-end.** The dialog's Save/Create/Delete emit
  `UpdatePrim` / `CreatePrim` / `DeletePrim` when connected (previously only gizmo drags
  sent anything). `CreatePrim` was extended to carry the full initial authoring state
  (name/transform/color/texture/surface/geometry) so online create has offline parity; the
  server inserts then applies it in one step and broadcasts `PrimUpsert`.
- The client now applies `PrimUpsert` broadcasts: existing prims update **in place**
  (mesh/texture/surface refreshed via marker components — no despawn/respawn flicker at the
  ~20 Hz gizmo-drag rate), and unknown ids spawn. The prim the local user currently has
  selected is skipped, since its local copy is authoritative during editing and was already
  applied locally — avoiding an echo fight.
- Selection highlight is a subtle additive emissive (not a base-color tint), so the texture
  stays visible while a prim is selected/edited.
