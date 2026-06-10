# Phase 1: Protocol — Implementation Complete

**Date**: 2025-05-08

## Summary

Phase 1 of ADR-017 (Prim Authoring and Editing) has been successfully implemented. This phase introduces the wire protocol messages and data structures needed for prim create, update, delete, and server-to-client replication.

## Changes Made

### 1. Protocol Version Bump
- **`vibe_core/src/protocol.rs`**: Bumped `PROTOCOL_VERSION` from `4` to `5`
- Enforced by handshake validation in `vibers-sim/src/net.rs` — incompatible clients will be rejected

### 2. New MessageKind Enum Variants
Added four new message kinds to `MessageKind`:

| Kind ID | Variant | Direction | Purpose |
|---------|---------|-----------|---------|
| 8 | `CreatePrim` | C→S | Request new prim at a position |
| 9 | `UpdatePrim` | C→S | Send updated prim state |
| 10 | `DeletePrim` | C→S | Request prim deletion |
| 11 | `PrimUpsert` | S→C | Broadcast prim after mutation |

Updated `MessageKind::from_wire()` to handle IDs 8–11.

### 3. New NetMessage Enum Variants

**`CreatePrim { request_id: u32, region_id: i64, position: Vec3, shape: String }`**
- Client sends when attempting to create a prim
- Server will validate region existence and return `PrimUpsert` with server-assigned id

**`UpdatePrim { request_id: u32, prim_id: i64, position: Vec3, rotation: Vec3, scale: Vec3, color: [f32; 3], texture_id: Option<String>, name: String }`**
- Client sends with updated field values
- Server persists and returns `PrimUpsert` with the authoritative state

**`DeletePrim { request_id: u32, prim_id: i64 }`**
- Client requests deletion
- Server removes and broadcasts `PrimRemoved { id }`

**`PrimUpsert { prim: PrimDto }`**
- Server broadcasts the full authoritative prim after create/update
- Sent via delta broadcast (not waiting for next full snapshot)
- Enables immediate replication to all connected clients

### 4. PrimDto Extension
Added `texture_id: Option<String>` field to `PrimDto` with `#[serde(default)]` for backward compatibility during rollout. This field:
- Stores a key from the standard texture library (e.g. `"brick"`, `"grass"`)
- Is `None` if the prim has no texture assigned
- Will be stored in `prims.texture_id` DB column (to be added in Phase 2 migration)

### 5. Helper Function Updates
- `message_kind()` — updated to handle new variants
- `message_request_id()` — updated to extract `request_id` from `CreatePrim`, `UpdatePrim`, `DeletePrim`

### 6. Server Message Handling
**`vibers-sim/src/net.rs`** — updated the per-client message match to:
- `CreatePrim { .. }` — logs "not yet implemented" (placeholder for Phase 3)
- `UpdatePrim { .. }` — logs "not yet implemented" (placeholder for Phase 3)
- `DeletePrim { .. }` — logs "not yet implemented" (placeholder for Phase 3)
- `PrimUpsert { .. }` — marked as "ignored from client" (server-only message)

### 7. Database Layer Compatibility
**`vibers-sim/src/db.rs`** — updated `load_world()` to:
- Select `texture_id` column from prims (will be added by Phase 2 migration)
- Initialize `texture_id` field when constructing `PrimDto` from database rows
- Using `row.get(16)?` to retrieve the texture_id (16th column in SELECT)

### 8. Tests
Added four comprehensive unit tests in `vibe_core/src/protocol.rs`:

- `roundtrip_create_prim_app_frame` — verifies `CreatePrim` serializes and deserializes correctly
- `roundtrip_update_prim_app_frame` — verifies `UpdatePrim` with all fields round-trips (including `texture_id`)
- `roundtrip_delete_prim_app_frame` — verifies `DeletePrim` round-trips
- `roundtrip_prim_upsert_app_frame` — verifies `PrimUpsert` with full `PrimDto` round-trips

All tests pass ✓

## Compilation Status

| Crate | Status |
|-------|--------|
| `vibe_core` | ✓ Compiles, 7 tests pass (including 4 new prim protocol tests) |
| `vibers-sim` | ✓ Compiles, message handling stubs in place |
| `vibers-rs` | ✓ Compiles |

## What's Next

### Phase 2 — Database: Prim Write Operations
- New migration `V2__prim_texture.sql` to add `texture_id` column to prims table
- `insert_prim()`, `update_prim()`, `delete_prim()` functions in `db.rs`
- Keep `Connection` alive in `main.rs` for runtime writes

### Phase 3 — Server: Prim Mutation Handling
- Implement `create_prim()`, `update_prim()`, `delete_prim()` on `SimWorld`
- Update net.rs handlers to call these methods and broadcast results
- Add delta broadcast channel for `PrimUpsert` / `PrimRemoved`

### Phases 4–7 — Client UI and Textures
- Raycasting-based prim selection
- Context menus (Create / Edit / Delete)
- Edit dialog with transform and texture picker
- Transform gizmos
- Bundled texture library and UI

## Wire Format Reference

All prim messages use the standard app-frame envelope (ADR-009):
```
[2 bytes: protocol_version = 5]
[2 bytes: message_kind (8, 9, 10, or 11)]
[4 bytes: request_id]
[variable: postcard-encoded NetMessage body]
```

Example `CreatePrim` payload:
```rust
CreatePrim {
    request_id: 123,
    region_id: 1,
    position: Vec3::new(10.0, 5.0, 20.0),
    shape: "box".to_string(),
}
```

Example `UpdatePrim` payload:
```rust
UpdatePrim {
    request_id: 124,
    prim_id: 42,
    position: Vec3::new(10.0, 5.0, 20.0),
    rotation: Vec3::new(0.0, 90.0, 0.0),
    scale: Vec3::new(2.0, 2.0, 2.0),
    color: [1.0, 0.0, 0.0],
    texture_id: Some("brick".to_string()),
    name: "My Prim".to_string(),
}
```

## Files Modified

| File | Changes |
|------|---------|
| `vibe-rs/crates/vibe_core/src/protocol.rs` | Protocol version, MessageKind, NetMessage, PrimDto, helper functions, tests |
| `vibe-rs/crates/vibers-sim/src/net.rs` | Message handlers for new variants |
| `vibe-rs/crates/vibers-sim/src/db.rs` | Updated `load_world()` to select and initialize `texture_id` |
| `vibe-rs/meta/adr/017-prim-authoring-and-editing-v0.md` | ADR document (new file) |
| `vibe-rs/meta/adr/index.md` | Added ADR-017 row |

## Breaking Changes

- **Protocol Version 5**: Old v4 clients will be rejected with `ServerError { code: 1, message: "version 4 not supported" }`
- **`PrimDto` schema**: Added `texture_id` field (but with `#[serde(default)]` for compatibility)

Both are expected at this stage of the project and handled gracefully.
