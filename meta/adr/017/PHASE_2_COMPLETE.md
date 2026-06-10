# Phase 2: Database — Implementation Complete

**Date**: 2025-05-08  
**Duration**: ~2 hours (as planned)

## Summary

Phase 2 of ADR-017 (Prim Authoring and Editing) has been successfully implemented. This phase adds database write operations for prim mutations, along with comprehensive unit tests and preparation for Phase 3 server handlers.

## What Was Implemented

### 1. Database Migration V2

**File**: `crates/vibers-sim/migrations/V2__prim_texture.sql`

```sql
ALTER TABLE prims ADD COLUMN texture_id TEXT DEFAULT NULL;
```

- Added optional `texture_id` column for storing texture library keys
- Uses `DEFAULT NULL` for backward compatibility with existing prims
- Automatically applied on first run via Refinery migration system
- Non-destructive; existing data is preserved

### 2. Prim Write Functions in db.rs

**Three new public functions** (ADR-017 Phase 2):

#### `insert_prim(conn, region_id, position, shape) -> Result<PrimDto>`

Creates a new prim with sensible defaults:
- **Name**: `"Prim"` (can be renamed via edit dialog)
- **Rotation**: `Vec3::ZERO`
- **Scale**: `Vec3::ONE` (unit size)
- **Color**: `[0.5, 0.5, 0.5]` (neutral gray)
- **Texture**: `None`

Returns the full `PrimDto` with server-assigned `id`.

**Validation**: Checks that the region exists before inserting.

**Example**:
```rust
let prim = insert_prim(&conn, region_id, Vec3::new(10.0, 5.0, 20.0), "box")?;
// Returns: PrimDto { id: 42, region_id, name: "Prim", shape: "box", ... }
```

#### `update_prim(conn, prim_id, position, rotation, scale, color, texture_id, name) -> Result<Option<PrimDto>>`

Updates all mutable fields of an existing prim:
- Position, rotation, scale (transforms)
- Color (RGB)
- Texture ID (from standard library)
- Name

Returns `Some(PrimDto)` on success, `None` if prim not found.

**Updates `updated_at` timestamp** for audit trails.

**Example**:
```rust
let updated = update_prim(
    &conn, prim_id,
    Vec3::new(15.0, 10.0, 25.0),
    Vec3::new(45.0, 90.0, 0.0),
    Vec3::new(2.0, 2.0, 2.0),
    [1.0, 0.0, 0.0],
    Some("brick".to_string()),
    "Red Box",
)?;
```

#### `delete_prim(conn, prim_id) -> Result<bool>`

Deletes a prim by id.

Returns `true` if a prim was deleted, `false` if not found.

**Example**:
```rust
let deleted = delete_prim(&conn, prim_id)?;
if deleted {
    println!("Prim removed");
}
```

#### `select_prim_by_id(conn, prim_id) -> Result<Option<PrimDto>>` (internal helper)

Private helper function used by the above functions to retrieve prim state by id.

### 3. Comprehensive Unit Tests

**8 tests added** to `db.rs` (all passing):

| Test | Purpose |
|------|---------|
| `test_insert_prim` | Verify prim creation with correct defaults |
| `test_insert_prim_invalid_region` | Ensure validation rejects non-existent regions |
| `test_update_prim` | Verify all fields are updated correctly |
| `test_update_prim_nonexistent` | Ensure update returns None for missing prims |
| `test_delete_prim` | Verify deletion removes the prim from DB |
| `test_delete_prim_nonexistent` | Ensure delete returns false for missing prims |
| `test_texture_roundtrip` | Verify texture_id persists through update/reload |
| `test_multiple_prims_isolation` | Ensure mutations on one prim don't affect others |

Each test:
- Creates a temporary SQLite database
- Runs migrations automatically
- Verifies database state after operations
- Uses the seeded "Groningen" region for testing
- Cleans up temporary files via `tempfile::TempDir`

**Test Results**:
```
running 8 tests
test db::tests::test_insert_prim ... ok
test db::tests::test_insert_prim_invalid_region ... ok
test db::tests::test_update_prim ... ok
test db::tests::test_update_prim_nonexistent ... ok
test db::tests::test_delete_prim ... ok
test db::tests::test_delete_prim_nonexistent ... ok
test db::tests::test_texture_roundtrip ... ok
test db::tests::test_multiple_prims_isolation ... ok

test result: ok. 8 passed; 0 failed; 0 ignored
```

### 4. SimWorld Preparation for Phase 3

**File**: `crates/vibers-sim/src/state.rs`

Added three method stubs (marked `#[allow(dead_code)]` until Phase 3):

#### `add_prim(&mut self, prim: PrimDto)`
- Appends a prim to the in-memory list
- Used after successful `db::insert_prim`

#### `update_prim(&mut self, prim_id: i64, updated_prim: PrimDto) -> bool`
- Updates a prim in-memory by id
- Returns `true` if found, `false` otherwise
- Used after successful `db::update_prim`

#### `remove_prim(&mut self, prim_id: i64) -> bool`
- Deletes a prim from the in-memory list by id
- Returns `true` if found, `false` otherwise
- Used after successful `db::delete_prim`

These methods coordinate with database operations: the server calls the database function first (for persistence), then the sim method (for in-memory state).

### 5. Dependency Addition

**Cargo.toml** for vibers-sim:
- Added `tempfile = "3.0"` as a dev dependency for test database creation

## Compilation & Testing Status

### Build Status

✅ **All crates compile**
- `vibe_core`: ✓
- `vibers-sim`: ✓ (4 warnings for Phase 3 dead code — expected)
- `vibers-rs`: ✓

### Test Results

**vibe_core**: 7/7 tests pass (Phase 1 tests unaffected)
```
test protocol::tests::roundtrip_create_prim_app_frame ... ok
test protocol::tests::roundtrip_delete_prim_app_frame ... ok
test protocol::tests::roundtrip_hello_app_frame ... ok
test protocol::tests::roundtrip_prim_upsert_app_frame ... ok
test protocol::tests::roundtrip_update_prim_app_frame ... ok
test protocol::tests::roundtrip_snapshot_app_frame ... ok
test world::tests::groningen_tile_roundtrip_rough ... ok

test result: ok. 7 passed; 0 failed
```

**vibers-sim**: 8/8 tests pass (all new)
```
test db::tests::test_delete_prim ... ok
test db::tests::test_delete_prim_nonexistent ... ok
test db::tests::test_insert_prim ... ok
test db::tests::test_insert_prim_invalid_region ... ok
test db::tests::test_multiple_prims_isolation ... ok
test db::tests::test_texture_roundtrip ... ok
test db::tests::test_update_prim ... ok
test db::tests::test_update_prim_nonexistent ... ok

test result: ok. 8 passed; 0 failed
```

**Total**: 15 tests pass, 0 failures

## Files Modified/Created

| File | Changes |
|------|---------|
| `migrations/V2__prim_texture.sql` | **NEW**: Migration adds texture_id column |
| `src/db.rs` | `insert_prim()`, `update_prim()`, `delete_prim()`, `select_prim_by_id()` + 8 tests |
| `src/state.rs` | `add_prim()`, `update_prim()`, `remove_prim()` method stubs |
| `Cargo.toml` | Added `tempfile = "3.0"` dev dependency |

## Lines of Code

| Subsystem | Lines |
|-----------|-------|
| V2 migration | 4 |
| Database functions (insert, update, delete, helper) | ~120 |
| Unit tests | ~150 |
| SimWorld stubs (Phase 3) | ~40 |
| **Total new code**: | ~314 |

## What's Ready for Phase 3

✅ **Database layer fully functional**
- Create, read, update, delete operations
- Comprehensive test coverage
- Texture support (`texture_id` column)
- Transaction semantics (each operation is atomic)

✅ **SimWorld preparation**
- Method stubs ready for server mutation handlers
- In-memory list management patterns defined
- Coordinate with database writes

❌ **Not yet implemented** (Phase 3):
- `net.rs` handlers to call these database functions
- Delta broadcast channel for `PrimUpsert` / `PrimRemoved`
- Request/response correlation (request_id tracking)

## Database Schema After Migration

```sql
CREATE TABLE prims (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    region_id INTEGER NOT NULL,
    name TEXT NOT NULL DEFAULT 'Prim',
    shape TEXT NOT NULL DEFAULT 'box',
    position_x REAL, position_y REAL, position_z REAL,
    rotation_x REAL, rotation_y REAL, rotation_z REAL,
    scale_x REAL, scale_y REAL, scale_z REAL,
    color_r REAL, color_g REAL, color_b REAL,
    texture_id TEXT DEFAULT NULL,          -- NEW in V2
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (region_id) REFERENCES regions(id) ON DELETE CASCADE
);
```

## Key Design Decisions

### Why separate database functions?

Following ADR-013 (server-as-sole-writer):
- All writes go through `db.rs` functions
- SimWorld doesn't directly modify the database
- Ensures transactional safety and audit trail

### Why return full `PrimDto`?

Phase 3 needs to broadcast the authoritative state immediately:
- After `insert_prim()`: server has the generated `id` to return to client
- After `update_prim()`: server has the confirmed state to broadcast
- After `delete_prim()`: server knows which `id` was removed (for `PrimRemoved` message)

### Why use `tempfile` for tests?

Each test needs an isolated database:
- No interference between tests
- Automatic cleanup via `TempDir` drop
- Full migration cycle (V1 + V2) runs on each test

## Performance Characteristics

**Database operations** (SQLite with Mutex):
- `insert_prim()`: 1 INSERT + 1 SELECT = 2 queries (µs range)
- `update_prim()`: 1 UPDATE + 1 SELECT = 2 queries
- `delete_prim()`: 1 DELETE = 1 query
- Mutex serializes writes (acceptable for v0 scale)

**No batching** in v0 (simplicity over throughput):
- Each mutation is one round-trip to disk
- Future optimization: batch writes per tick if analysis shows bottleneck

## Blocking Issues for Phase 3

✓ All Phase 2 deliverables complete
✓ Database API stable and tested
✓ SimWorld preparation in place

Phase 3 can proceed immediately:
1. Wire up `net.rs` handlers
2. Call `db::*` functions from handlers
3. Call `SimWorld::*` methods to update in-memory state
4. Broadcast `PrimUpsert` / `PrimRemoved` to all clients

Estimated Phase 3 effort: **~2 hours** (same as Phase 2)

## Notes for Future Phases

### Connection Lifetime (for Phase 3)

Currently, `main.rs` keeps the Connection alive via:
```rust
let conn = Arc::new(Mutex::new(open_and_migrate(...)?));
```

For Phase 3, pass this to client handlers:
```rust
tokio::spawn(handle_connection(stream, world, config, conn.clone(), rx));
```

Then in `handle_connection`, use:
```rust
let prim = db::insert_prim(&*conn.lock().unwrap(), ...)?;
```

### Texture Library (for Phase 7)

The `texture_id` field is ready for Phase 7:
- Database column stores string keys
- PrimDto serializes/deserializes correctly
- Tests verify roundtrip persistence
- UI in Phase 5 will have a texture picker dropdown

---

## Summary

✅ **Phase 2 is complete and ready for Phase 3 server integration**

**Deliverables**:
- V2 migration (texture_id column)
- Three database CRUD functions with full error handling
- 8 unit tests (100% pass rate)
- SimWorld stub methods for in-memory state
- Comprehensive documentation

**Next step**: See `PHASE_3_PLAN.md` (or read ADR-017 Phase 3 section in README.md)
