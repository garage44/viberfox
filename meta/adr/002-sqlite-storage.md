# ADR-002: SQLite for Region and Prim Storage

---
**Metadata:**
- **ID**: ADR-002
- **Status**: Accepted
- **Date**: 2025-03-18
- **Tags**: [storage, database]
- **Decision Type**: technology_choice
- **Related**: []
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Need persistent storage for regions (geographic areas with tile coordinates) and prims (3D primitives: boxes, spheres, cylinders, cones, toruses) with position, rotation, scale, and color.

**Requirements**:
- Local persistence
- Simple schema (regions, prims with FK to regions)
- No extra server process
- Rust ecosystem (rusqlite)

## Decision

Use SQLite via rusqlite for region and prim storage.

**Approach**: Single file `data/regions.db` with `regions` and `prims` tables. Indexed by tile coordinates and region_id. Seed default region (Groningen) if empty.

## Rationale

**Primary Reasoning**:
1. **Zero-config**: Single file, no server
2. **Mature**: SQLite is battle-tested and reliable
3. **rusqlite**: Rust bindings, no extra dependencies
4. **ACID**: Transactions for consistency
5. **Portable**: Copy `data/` folder to move data

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| JSON files | Simple | No schema, no querying | Need relational queries |
| PostgreSQL | Full-featured | Requires server | Overkill for local use |
| sled | Embedded, Rust | Key-value | Need relational model |
| SQLite | Embedded, SQL | Single writer | Acceptable for single-user |

## Consequences

**Positive**:
- Simple schema in `src/db/schema.rs`
- Easy to inspect with `sqlite3 data/regions.db`
- Migrations possible via SQL scripts

**Negative**:
- Single writer (mitigation: acceptable for single-user client)
- No built-in replication (mitigation: out of scope for now)

## Related

- Schema: `src/db/schema.rs`
- Systems: `src/systems/database.rs`
