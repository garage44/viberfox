# Phase 2: Database — Prim Write Operations (Planning)

**ADR**: ADR-017 Prim Authoring and Editing (v0)

## Overview

Phase 2 implements the database layer for prim mutations. This includes:
1. A new database migration to add the `texture_id` column
2. Three new database functions: `insert_prim()`, `update_prim()`, `delete_prim()`
3. Keeping the SQLite connection alive at runtime (instead of opening once and dropping)

## What Phase 1 Set Up

- **Protocol messages**: `CreatePrim`, `UpdatePrim`, `DeletePrim`, `PrimUpsert` (kinds 8–11)
- **Data structure**: `PrimDto` now includes `texture_id: Option<String>`
- **Server stubs**: `net.rs` has placeholder handlers that log "not yet implemented"
- **Database readiness**: `db.rs` already tries to read `texture_id` from the prims table (added `row.get(16)?`)

## Phase 2 Implementation Tasks

### Task 1: Create Migration V2

**File**: `vibe-rs/crates/vibers-sim/migrations/V2__prim_texture.sql` (new)

```sql
ALTER TABLE prims ADD COLUMN texture_id TEXT DEFAULT NULL;
```

**Why**: Store the texture library key (e.g., `"brick"`, `"grass"`) alongside prim data. The column is nullable (`DEFAULT NULL`) to handle prims created before textures were supported.

### Task 2: Add Database Write Functions

**File**: `vibe-rs/crates/vibers-sim/src/db.rs`

#### `insert_prim(conn: &Connection, region_id: i64, position: Vec3, shape: &str) -> anyhow::Result<PrimDto>`

**Logic**:
1. Validate that `region_id` exists in the regions table
2. `INSERT INTO prims (region_id, name, shape, position_x, position_y, position_z, rotation_x, rotation_y, rotation_z, scale_x, scale_y, scale_z, color_r, color_g, color_b, texture_id, created_at, updated_at) VALUES (...)`
3. Retrieve the generated `id` via `last_insert_rowid()` or `RETURNING id`
4. `SELECT ... WHERE id = ?` to construct and return the full `PrimDto`

**Defaults**:
- `name`: `"Prim"` (can be renamed in edit dialog)
- `rotation`: `Vec3::ZERO`
- `scale`: `Vec3::ONE`
- `color`: `[0.5, 0.5, 0.5]` (neutral gray)
- `texture_id`: `NULL`

#### `update_prim(conn: &Connection, prim_id: i64, position: Vec3, rotation: Vec3, scale: Vec3, color: [f32; 3], texture_id: Option<String>, name: &str) -> anyhow::Result<Option<PrimDto>>`

**Logic**:
1. Verify prim exists: `SELECT id FROM prims WHERE id = ?`
2. If not found, return `Ok(None)`
3. `UPDATE prims SET position_x = ?, position_y = ?, position_z = ?, rotation_x = ?, ..., texture_id = ?, name = ?, updated_at = datetime('now') WHERE id = ?`
4. `SELECT ... WHERE id = ?` to construct and return the updated `PrimDto`

**Note**: All fields are updatable; the client is responsible for only sending changed fields (or re-sending full state).

#### `delete_prim(conn: &Connection, prim_id: i64) -> anyhow::Result<bool>`

**Logic**:
1. `DELETE FROM prims WHERE id = ?`
2. Return `true` if a row was deleted, `false` otherwise (can use `changes()` API or check the result)

### Task 3: Keep Connection Alive

**File**: `vibe-rs/crates/vibers-sim/src/main.rs`

**Current flow**:
```rust
let conn = db::open_and_migrate(&config.database_path)?;
let (regions, prims) = db::load_world(&conn)?;
drop(conn);  // ← connection is dropped here
```

**New flow**:
```rust
let conn = Arc::new(Mutex::new(
    db::open_and_migrate(&config.database_path)?
));
let (regions, prims) = db::load_world(&conn.lock().unwrap())?;
// conn is moved into Arc and shared with client handlers
```

**Pass to client handlers**:
- Each `tokio::spawn(handle_connection(...))` call receives `conn.clone()`
- Inside `handle_connection()`, mutations can now call `db::insert_prim()`, etc. while holding `conn.lock().unwrap()`

**Concurrency**:
- SQLite's locking is fine with multiple readers + single writer
- `Mutex<Connection>` serializes all writes (acceptable for v0)
- Reads (snapshots) can happen concurrently with active client handler tasks (they don't hold the lock during `read()`)

### Task 4: Database Function Signatures in state.rs

**File**: `vibe-rs/crates/vibers-sim/src/state.rs`

While Phase 2 is mainly about database, the `SimWorld` struct will eventually call these functions. For now, we can add stub signatures or notes:

```rust
impl SimWorld {
    /// Not yet called; Phase 3 will wire this up.
    /// Returns the created prim with server-assigned id.
    pub fn create_prim(&mut self, conn: &Connection, region_id: i64, position: Vec3, shape: &str) -> anyhow::Result<PrimDto> {
        let prim = db::insert_prim(conn, region_id, position, shape)?;
        self.prims.push(prim.clone());
        Ok(prim)
    }
    
    // Similar for update_prim and delete_prim
}
```

Or defer these methods until Phase 3.

## Testing Phase 2

### Unit Tests
1. **`db::tests::test_insert_prim`** — Create a prim, verify it has a non-zero id, is findable in the table
2. **`db::tests::test_update_prim`** — Update a prim, verify fields changed, texture_id persisted
3. **`db::tests::test_delete_prim`** — Delete a prim, verify it no longer exists
4. **`db::tests::test_texture_roundtrip`** — Create prim with texture_id, read it back, verify texture_id matches

### Manual Integration Test
1. Run `vibers-sim` (with Phase 2 migration auto-applied)
2. Check that `data/regions.db` has the `texture_id` column on the prims table
3. Verify that existing prims load without errors (backward compat with `texture_id = NULL`)

## Estimated Effort

- **Migration file**: ~1 minute
- **`insert_prim()`**: ~15 minutes (SQL + error handling + validation)
- **`update_prim()`**: ~15 minutes
- **`delete_prim()`**: ~10 minutes
- **Connection refactor in main.rs**: ~10 minutes
- **Tests**: ~30 minutes
- **Testing & debugging**: ~30 minutes

**Total**: ~2 hours of development

## Deliverables

✓ `V2__prim_texture.sql` migration
✓ `db::insert_prim()`, `db::update_prim()`, `db::delete_prim()` functions
✓ `Connection` kept alive in `Arc<Mutex<>>` and passed to client handlers
✓ Unit tests for all three functions
✓ Project compiles and tests pass
✓ Database backward-compatible (old prims load fine with `texture_id = NULL`)

## Blocking Issues for Phase 3

Once Phase 2 is done, Phase 3 will:
1. Call `db::*` functions from within `SimWorld::*` methods
2. Update `net.rs` handlers to call `world.create_prim()`, etc.
3. Broadcast `PrimUpsert` / `PrimRemoved` via a new delta broadcast channel

Phase 2 is the blocking prerequisite for all of the above.

## Decision Log

- **Why not async db access?** SQLite in Rust is sync-only (rusqlite). Phase 3 will handle tokio integration via `tokio::task::block_in_place` if needed, or keep the current Mutex pattern (simpler for v0).
- **Why `Arc<Mutex<>>` instead of `RwLock`?** SQLite has its own internal locking. Using a simple `Mutex` is clearer and avoids potential deadlock scenarios with nested locks.
- **Why no batch writes?** v0 prioritizes simplicity. Each prim mutation is one write. Future phases can batch if performance analysis shows a need.
