# ADR-017: Prim Authoring and Editing (v0)

---
**Metadata:**
- **ID**: ADR-017
- **Status**: Proposed
- **Date**: 2025-07-14
- **Tags**: authoring, prims, ui, protocol, persistence, textures, replication
- **Decision Type**: architecture_pattern
- **Aligns with**: G-01 (authoring), G-02 (visiting / consistent replicated world), P-01 (authoritative sim), P-02 (explicit protocol), P-04 (security by default for mutations), P-05 (modular boundaries)
- **Roadmap tier**: P0 — core authoring loop
- **Doc resolution**: R2 — trade-offs matter; multiple layers affected end-to-end
- **Related**: ADR-002 (SQLite storage), ADR-009 (protocol envelope), ADR-011 (world replication), ADR-013 (migrations & server writer), ADR-015 (workspace boundaries)
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Users cannot create, edit, or delete primitive objects ("prims") in the shared virtual world. The current prim pipeline is **read-only**: prims are loaded from SQLite at server startup, broadcast inside `WorldSnapshot`, and rendered on clients — but no mutation path exists at any layer (protocol, server state, database writes, or client UI).

This is a hard blocker for **G-01 (authoring)**: without the ability to place and modify objects, the world is static and the core product loop is broken.

**Current state** (what already exists):

| Layer | What works |
|-------|-----------|
| **Protocol** (`vibe_core`) | `PrimDto` with id, region_id, name, shape, position, rotation, scale, color. `WorldSnapshot` includes `prims: Vec<PrimDto>`. `PrimRemoved` (kind 7) server→client variant exists. |
| **Database** (`vibers-sim`) | `prims` table in SQLite (V1 migration) with full positional/color columns, FK to regions. |
| **Server state** (`state.rs`) | `SimWorld` holds `prims: Vec<PrimDto>`, serves them in `snapshot()`. |
| **Client rendering** (`vibers-rs`) | `Prim` component + `PrimShape` enum. `spawn_prims` creates meshes from shape/color. `GameState::selected_prim_id` placeholder (dead code). |

**What is completely missing** (end-to-end):

| Capability | Protocol | Server net | Sim state | DB | Client |
|-----------|----------|-----------|-----------|-----|--------|
| Prim create (client→server) | ❌ | ❌ | ❌ | ❌ | ❌ |
| Prim update (client→server) | ❌ | ❌ | ❌ | ❌ | ❌ |
| Prim delete (client→server) | ❌ | ❌ (ignored) | ❌ | ❌ | ❌ |
| Prim delta broadcast (server→clients) | ❌ | ❌ | ❌ | — | ❌ |
| Prim selection / raycasting | — | — | — | — | ❌ |
| Context menu UI | — | — | — | — | ❌ |
| Transform gizmos | — | — | — | — | ❌ |
| Edit dialog / texture assignment | — | — | — | — | ❌ |
| Texture field on prims | ❌ | — | ❌ | ❌ | ❌ |
| Standard texture library | — | — | — | — | ❌ |

**Requirements**:
- **R1**: Authenticated client can create a prim on a region via right-click → "Create prim".
- **R2**: Client can select an existing prim (click) and open "Edit prim" (right-click context menu).
- **R3**: Edit mode shows translate / rotate / scale gizmo handles; changes replicate live to all clients.
- **R4**: Edit dialog allows assigning a texture from a standard library.
- **R5**: All mutations are server-authoritative (P-01) and persisted to SQLite (ADR-013).
- **R6**: Other connected clients see prim changes within one tick (~50 ms at 20 Hz).
- **R7**: Prim deletion is supported with confirmation.

## Decision

**Implement prim authoring as a seven-phase plan spanning all three crates (`vibe_core`, `vibers-sim`, `vibers-rs`), adding protocol messages for prim CRUD, server mutation logic with DB persistence, client UI via `bevy_egui`, raycasting-based selection, transform gizmos, and a bundled texture library.**

### Phase 1 — Protocol: prim mutation messages (`vibe_core`)

Add four new `NetMessage` variants:

| Variant | Direction | Kind ID | Purpose |
|---------|-----------|---------|---------|
| `CreatePrim` | C→S | 8 | Request a new prim at a position within a region |
| `UpdatePrim` | C→S | 9 | Push updated prim fields (position, rotation, scale, color, texture, name) |
| `DeletePrim` | C→S | 10 | Request prim deletion by server-assigned id |
| `PrimUpsert` | S→C | 11 | Server broadcasts the full authoritative prim state after a create or update |

Bump `PROTOCOL_VERSION` to `5`.

**Payloads:**

```
CreatePrim {
    request_id: u32,
    region_id: i64,
    position: Vec3,        // click-point in sim-space
    shape: String,         // "box" | "sphere" | "cylinder" | "cone" | "torus"
}

UpdatePrim {
    request_id: u32,
    prim_id: i64,
    position: Vec3,
    rotation: Vec3,
    scale: Vec3,
    color: [f32; 3],
    texture_id: Option<String>,   // key from standard texture library, or None
    name: String,
}

DeletePrim {
    request_id: u32,
    prim_id: i64,
}

PrimUpsert {
    prim: PrimDto,         // full prim state after server applies the mutation
}
```

The existing `PrimRemoved` (kind 7, S→C) becomes actively emitted by the server on successful deletion.

**`PrimDto` extension**: add `pub texture_id: Option<String>` field with `#[serde(default)]` for wire compatibility during rollout.

**Files changed**: `vibe_core/src/protocol.rs`, `vibe_core/src/error.rs` (if new error variants needed).

### Phase 2 — Database: prim write operations (`vibers-sim`)

**New migration** `V2__prim_texture.sql`:

```
ALTER TABLE prims ADD COLUMN texture_id TEXT DEFAULT NULL;
```

**New functions** in `db.rs`:

| Function | SQL | Returns |
|----------|-----|---------|
| `insert_prim(conn, region_id, position, shape)` | `INSERT INTO prims (...) VALUES (...)` + `SELECT ... WHERE id = last_insert_rowid()` | `PrimDto` |
| `update_prim(conn, prim_id, fields)` | `UPDATE prims SET ... WHERE id = ?` + `SELECT` | `Option<PrimDto>` |
| `delete_prim(conn, prim_id)` | `DELETE FROM prims WHERE id = ?` | `bool` |

All write functions return the full `PrimDto` (or confirmation) so the server can broadcast authoritative state.

**Database access change**: The `Connection` is currently opened once in `main.rs` and dropped after `load_world`. It must be kept alive (wrapped in `Arc<Mutex<Connection>>` or a similar holder) and passed to the client handler tasks so they can issue writes when processing mutation messages.

**Files changed**: `vibers-sim/migrations/V2__prim_texture.sql` (new), `vibers-sim/src/db.rs`, `vibers-sim/src/main.rs` (connection lifetime).

### Phase 3 — Server: prim mutation handling (`vibers-sim`)

**`state.rs` additions:**

| Method | Behaviour |
|--------|-----------|
| `create_prim(region_id, position, shape) -> PrimDto` | Validates region exists, writes to DB, appends to in-memory `prims` vec, returns authoritative DTO. |
| `update_prim(prim_id, fields) -> Option<PrimDto>` | Finds prim, writes to DB, updates in-memory vec entry, returns DTO. |
| `delete_prim(prim_id) -> bool` | Removes from in-memory vec, deletes from DB, returns success. |

> Note: `SimWorld` must gain a reference to the DB connection (e.g. `Arc<Mutex<Connection>>`) so mutations persist immediately rather than relying on snapshot-to-DB sync.

**`net.rs` additions** (inside the per-client `match msg` block):

```
NetMessage::CreatePrim { request_id, region_id, position, shape } => {
    let prim = world.write().await.create_prim(region_id, position, &shape);
    // broadcast PrimUpsert to all clients via tx_snap or a dedicated delta channel
}
NetMessage::UpdatePrim { request_id, prim_id, ... } => {
    if let Some(prim) = world.write().await.update_prim(prim_id, ...) {
        // broadcast PrimUpsert
    } else {
        // send ServerError { request_id, code: NOT_FOUND, ... }
    }
}
NetMessage::DeletePrim { request_id, prim_id } => {
    if world.write().await.delete_prim(prim_id) {
        // broadcast PrimRemoved { id: prim_id }
    } else {
        // send ServerError
    }
}
```

**Delta broadcast**: Introduce a second `broadcast::channel<Vec<u8>>` (or reuse the existing one with a wrapper enum) for prim delta messages (`PrimUpsert` / `PrimRemoved`). These are sent immediately on mutation — they do not wait for the next tick. The full `WorldSnapshot` continues to include the current prim list as before (consistency fallback).

**Files changed**: `vibers-sim/src/state.rs`, `vibers-sim/src/net.rs`, `vibers-sim/src/main.rs`.

### Phase 4 — Client: selection and raycasting (`vibers-rs`)

**Dependency**: Add `bevy_mod_raycast` (or use Bevy 0.16's built-in picking if available) for GPU/mesh-based raycasting from the mouse cursor into the 3D scene.

**New system** `prim_picking`:
- On **left-click** (without drag): cast a ray from the camera through the cursor. If it hits an entity with `PrimMesh`, set `GameState::selected_prim_id = Some(prim.id)` and add a `Selected` marker component (with a highlight outline or wireframe overlay). If it hits nothing, deselect.
- On **right-click**: cast the same ray. If it hits a `PrimMesh`, show context menu with **"Edit prim"** and **"Delete prim"**. If it hits a `RegionMesh` (ground), show context menu with **"Create prim"**. Store the hit point in a resource (`ContextMenuState`) for the create position.

**New component**: `Selected` marker on the actively selected prim entity.

**New resource**: `ContextMenuState { visible: bool, screen_pos: Vec2, hit_point: Option<Vec3>, hit_prim_id: Option<i64>, hit_region_id: Option<i64> }`.

**Files changed**: `vibers-rs/src/systems/picking.rs` (new), `vibers-rs/src/components.rs`, `vibers-rs/src/resources.rs`.

### Phase 5 — Client: context menu and edit dialog (`vibers-rs`)

**Dependency**: Add `bevy_egui` for immediate-mode UI panels.

**Context menu** (rendered by an egui system each frame when `ContextMenuState::visible`):
- Positioned at `screen_pos`.
- Items:
  - **"Create prim"** → sends `CreatePrim { region_id, position: hit_point, shape: "box" }` to server via `OnlineSession::intent_tx`. After `PrimUpsert` response arrives, auto-selects the new prim and opens the edit dialog.
  - **"Edit prim"** → opens the edit dialog for `hit_prim_id`.
  - **"Delete prim"** → sends `DeletePrim { prim_id }` to server. Optionally show a confirmation sub-menu.

**Edit dialog** (egui window, open when `GameState::editing_prim` is `Some(prim_id)`):
- Fields:
  - **Name**: text input.
  - **Shape**: dropdown (Box / Sphere / Cylinder / Cone / Torus). Changing shape re-creates the mesh.
  - **Position**: Vec3 drag inputs (also manipulated via gizmo — see Phase 6).
  - **Rotation**: Vec3 drag inputs (degrees).
  - **Scale**: Vec3 drag inputs.
  - **Color**: RGB colour picker.
  - **Texture**: thumbnail grid of standard textures (see Phase 7). Click to assign; click again to clear.
- On any field change: immediately send `UpdatePrim` to the server. This gives live replication (R3, R6).
- **"Close"** button: deselects prim, removes `Selected` marker, closes dialog.

**New resource**: `EditDialogState { prim_id: i64, name: String, shape: String, position: Vec3, rotation: Vec3, scale: Vec3, color: [f32; 3], texture_id: Option<String> }`.

**Files changed**: `vibers-rs/src/systems/ui.rs` (new), `vibers-rs/src/resources.rs`, `vibers-rs/Cargo.toml`.

### Phase 6 — Client: transform gizmos (`vibers-rs`)

**Dependency**: Add `bevy_transform_gizmo` (or `transform-gizmo-bevy` crate) for visual translate/rotate/scale handles.

**Behaviour**:
- When a prim has the `Selected` marker, attach gizmo components (translate handles by default).
- Toolbar or keyboard shortcuts to switch mode: **G** = translate, **R** = rotate, **S** = scale (familiar from Blender).
- On each gizmo interaction frame, read the updated `Transform` and send `UpdatePrim` to the server (throttled to max 20 Hz to match tick rate — no point sending faster than the sim processes).
- Deselecting removes the gizmo components.

**Interaction with edit dialog**: Gizmo changes update the numeric fields in the dialog in real-time, and vice versa (dialog field edits update the gizmo).

**Files changed**: `vibers-rs/src/systems/gizmo.rs` (new), `vibers-rs/Cargo.toml`.

### Phase 7 — Standard texture library (`vibers-rs` + `assets/`)

**Approach**: Ship a set of **bundled textures** as PNG files under `assets/textures/standard/`. These are loaded at startup into a `TextureLibrary` resource.

**Texture set** (v0 — ~12–16 textures, 256×256 px each):

| Category | Textures |
|----------|----------|
| **Natural** | grass, dirt, sand, rock, water |
| **Built** | brick, concrete, wood_planks, metal, tile_floor |
| **Colours** | white, red, blue, green |
| **Patterns** | checkerboard, grid |

Each texture is identified by a **string key** matching its filename stem (e.g. `"brick"`, `"grass"`). This key is stored in `PrimDto::texture_id` and the `prims.texture_id` DB column.

**`TextureLibrary` resource**:
```
TextureLibrary {
    entries: Vec<TextureEntry>,         // ordered for UI display
    handles: HashMap<String, Handle<Image>>,  // key -> loaded Bevy image
}
struct TextureEntry {
    key: String,
    display_name: String,
    thumbnail: Handle<Image>,           // same handle, used for egui thumbnail
}
```

**Rendering change**: In `spawn_prims` / the update path, when `prim.texture_id` is `Some(key)`, look up the `Handle<Image>` from `TextureLibrary` and set it as the `base_color_texture` on the `StandardMaterial` (same pattern as region tile textures).

**Sourcing textures**: For v0, use CC0 / public-domain textures (e.g. from ambientCG, Poly Haven, or hand-authored simple tiles). Exact assets can be swapped later without architectural changes.

**Files changed**: `assets/textures/standard/` (new directory + PNGs), `vibers-rs/src/resources.rs` (`TextureLibrary`), `vibers-rs/src/systems/rendering.rs` (texture material path).

---

### Implementation order and dependencies

```
Phase 1 (protocol)
    |
    +---> Phase 2 (database) ---> Phase 3 (server)
    |                                  |
    +---> Phase 4 (picking) -----> Phase 5 (UI) ---> Phase 6 (gizmos)
    |
    +---> Phase 7 (textures) --- merges into Phase 5 (texture picker in dialog)
```

- **Phase 1** is prerequisite for everything (shared types).
- **Phases 2+3** (server-side) and **Phases 4+7** (client-side, no server dependency for initial scaffolding) can proceed in **parallel**.
- **Phase 5** depends on Phase 3 (server must accept mutations) and Phase 4 (picking must work).
- **Phase 6** depends on Phase 5 (needs the selection + dialog infrastructure).
- **Phase 7** can start early (asset preparation) and merges into Phase 5 for the texture picker.

**Estimated scope**: ~2,500–3,500 lines of new/changed Rust across the three crates, plus ~12–16 texture assets.

## Rationale

**Primary Reasoning**:
1. **Server-authoritative mutations (P-01)**: All prim state changes flow client→server→broadcast. The client never optimistically modifies shared state. This avoids desync and respects the authority model established in ADR-010/011.
2. **Immediate delta broadcast**: Using `PrimUpsert` / `PrimRemoved` messages on mutation (not waiting for the next full `WorldSnapshot`) gives sub-tick latency for edits. The full snapshot remains the consistency fallback.
3. **`bevy_egui` for UI**: The only mature immediate-mode UI integration for Bevy. Avoids building a custom retained-mode UI system. Widely used in Bevy projects for editor-style interfaces.
4. **String-keyed texture library**: Simple, human-readable identifiers that survive serialization and database storage. No UUIDs or asset hashes needed for a bundled standard library.
5. **Phased plan**: Allows incremental delivery and testing. Each phase produces a testable milestone (e.g. after Phase 3, you can test prim CRUD via a raw TCP client; after Phase 5, full UI flow).

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Client-authoritative prims (edit locally, sync later) | Lower latency for edits, simpler client code | Breaks P-01, desync risk with multiple editors, conflict resolution needed | Violates the core authority principle; untenable for multi-user editing |
| Full snapshot for prim changes (no delta messages) | No new protocol messages needed | 50ms latency floor, wastes bandwidth re-sending unchanged prims every tick | Already have `PrimRemoved` as precedent for delta; authoring UX needs responsiveness |
| Custom UI (Bevy native `bevy_ui`) | No extra dependency | Bevy's built-in UI is layout-oriented, not suited for editor dialogs (no text inputs, dropdowns, colour pickers) | Would require building dozens of widgets from scratch; `bevy_egui` provides them |
| ImGui via `bevy_imgui` | Familiar C++ ecosystem | Less maintained for Bevy 0.16, Rust bindings are thinner than egui | `bevy_egui` has better Bevy integration and is pure Rust |
| Upload custom textures (user assets) | More powerful | Requires asset pipeline, storage, CDN, abuse moderation (P-04) — entire ADR-worth of work | Deferred to a future "Asset Pipeline" ADR; v0 uses bundled textures only |
| Collaborative locking (only one editor at a time per prim) | Prevents conflicts | UX friction, complex lock management, timeout/crash recovery | Overkill for v0; last-write-wins on server is acceptable at low user counts |

## Consequences

**Positive**:
- Enables the core authoring loop (G-01): users can create, position, scale, rotate, texture, and delete objects in a shared world.
- Other clients see edits in real-time (G-02) — the world is live and collaborative.
- The pattern (client intent → server mutation → delta broadcast) established here will be reused for future mutation types (region editing, scripting, LLM-driven changes).
- The texture library provides immediate visual variety without requiring an asset upload pipeline.
- Phased delivery means value is shipped incrementally — even Phase 3 alone (server CRUD without UI) unblocks automated/scripted world building.

**Negative**:
- **New dependency (`bevy_egui`)**: adds ~15 crate transitive deps. Mitigation: widely used, actively maintained, acceptable for an editor-style client.
- **New dependency (gizmo crate)**: less mature than `bevy_egui`. Mitigation: can be replaced later; the gizmo is an optional UX enhancement (Phase 6), not a blocker for basic editing.
- **`PROTOCOL_VERSION` bump to 5**: requires coordinated client+server deploys. Mitigation: expected at this stage; version check is already enforced.
- **DB connection kept alive**: the current load-once-and-drop pattern must change to support runtime writes. Mitigation: straightforward `Arc<Mutex<Connection>>` pattern; SQLite handles concurrent reads fine and writes are serialized by the mutex.
- **Last-write-wins conflicts**: two users editing the same prim simultaneously will see the last `UpdatePrim` win. Mitigation: acceptable at v0 scale; collaborative locking can be layered on later.
- **Bundled textures increase binary/asset size**: ~12–16 PNGs at 256×256 ≈ 2–4 MB total. Mitigation: negligible; can be compressed further or lazy-loaded if needed.

## Related

- [ADR-002](./002-sqlite-storage.md): SQLite is the persistence layer for prims; Phase 2 adds write operations.
- [ADR-009](./009-application-protocol-envelope-v0.md): Protocol envelope carries the new message kinds (8–11).
- [ADR-011](./011-static-world-replication-v0.md): World replication; this ADR graduates prims from "static" to "mutable" with delta messages.
- [ADR-013](./013-sqlite-migrations-and-server-writer.md): Server-as-sole-writer principle; Phase 2 adds a new migration and write functions.
- [ADR-015](./015-workspace-module-boundaries.md): Shared types in `vibe_core`, mutation logic in `vibers-sim`, UI in `vibers-rs`.
