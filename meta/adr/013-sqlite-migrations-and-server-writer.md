# ADR-013: SQLite Migrations and Single Server Writer

---
**Metadata:**
- **ID**: ADR-013
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [sqlite, storage, migrations, server]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-002, ADR-007, ADR-011]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: [ADR-002](./002-sqlite-storage.md) chose SQLite for regions and prims. A networked sim (ADR-007) must avoid **corruption** from concurrent writers and must **evolve schema** without breaking deployed worlds.

**Requirements**:
- **Exactly one** process opens the DB for **writes** in production (the sim)
- **Versioned migrations** forward (and optionally documented rollback strategy)
- Clients **never** mount the remote SQLite file; they receive **DTOs** (ADR-011)

## Decision

**`vibe_sim`** is the **sole writer** to `regions.db` (or configured path). Schema changes go through a **migration** mechanism checked in alongside code.

**Rust ecosystem**:
- Keep **`rusqlite`** (ADR-002); add **`refinery`** with **embedded SQL migrations** (`include_str!` or refinery’s **`embed_migrations!`** helper) for versioned, transactional upgrades—common pairing in the ecosystem.
- Run migrations on sim startup **before** binding the network port; surface errors with **`anyhow`/`tracing`** at the binary.

**Approach**:
- **Connection discipline**: Client binary does not open the production DB path when in networked mode; local single-player may keep current behavior until explicitly deprecated.
- **Migrations**: One folder or module of ordered SQL/Rust migrations; sim applies on startup before accepting clients.
- **Backup hook**: Document operator expectation (copy `.db` before major upgrade) in ops ADR-014.

## Rationale

**Primary Reasoning**:
1. **SQLite expectation**: Single writer is the safe default; WAL helps readers but not multi-writer apps.
2. **Operational clarity**: Matches dedicated region sim mental model (OpenSim/Tundra server owns world).
3. **Growth**: Migrations unlock new columns for networking ids, timestamps, etc.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Multi-writer SQLite | Theoretical scale | Fragile | Rejected |
| Client direct DB over NFS | “Simple” | Corruption + latency | Rejected |
| Move to Postgres (**`sqlx`**) in v0 | Robust multi-node | Big jump | Defer until needed |

## Consequences

**Positive**:
- Clear ownership with ADR-007/011
- Repeatable upgrades

**Negative**:
- Migration mistakes brick data (mitigation: tests + backup docs)
- Single-node DB until replicated storage ADR (future)

## Related

- [ADR-002](./002-sqlite-storage.md): Original storage choice
- [ADR-007](./007-simulation-vs-client-process-model.md): Sim owns DB
- [ADR-011](./011-static-world-replication-v0.md): Clients get DTOs, not SQL
