# ADR-011: Static World Replication (v0)

---
**Metadata:**
- **ID**: ADR-011
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [replication, networking, regions, prims]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-002, ADR-009, ADR-010, ADR-012, ADR-013]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Clients need **regions** and **prims** from the authoritative store ([ADR-002](./002-sqlite-storage.md)) without opening SQLite over the network (ADR-013). Classic Tundra optimizes **per-connection sync** with **dirty attribute bitfields** and **full vs incremental** component sends ([`SyncState.h` pattern](https://github.com/realXtend/tundra/blob/master/src/Core/TundraProtocolModule/SyncState.h)).

**Requirements**:
- **Snapshot** on join (or after handshake, ADR-009)
- **Minimal deltas** for v0 (coarse-grained acceptable)
- **Server-assigned ids** for any entity created at runtime (pattern from [UniqueIdGenerator](https://github.com/realXtend/tundra/blob/master/src/Core/TundraCore/Scene/UniqueIdGenerator.h))
- No **CRDT** / full distributed editing in this ADR

## Decision

Replicate **static world** as **DTOs** (regions + prims) over ADR-009: initial **WorldSnapshot** message(s), then **small delta** messages (prim upsert/remove, region invalidate/reload) as implementation allows.

**Approach**:
- **Snapshot**: Bounded size; paginate or stream multiple frames if needed (document max).
- **Ids**: Integer ids stable in DB; server never trusts client-proposed ids for **new** objects without assignment ack.
- **Delta v0**: Prefer whole-prim replace or delete-by-id before field-level bitfields.
- **Evolution path**: Document that future work may adopt Tundra-style **per-client dirty masks** for fine-grained components (out of scope here).

**Rust ecosystem**:
- **`serde`** snapshot/delta DTOs in `vibe_core`; encode with the same codec as ADR-009 (**`postcard`** / **`bincode`**).
- Large static payloads: optional **`bytes::Bytes`** (or `Vec<u8>` v0) for opaque blobs if snapshots include mesh/asset data later.
- When AOI grows complex, consider **`rstar`** or **`spade`** for spatial indexes over region bounds (defer until profiling says linear scan fails).

## Rationale

**Primary Reasoning**:
1. **Matches current data model**: Regions and prims already map to SQLite rows.
2. **Fast to ship**: DTO snapshot is simpler than full EC attribute replication.
3. **Proven direction**: Tundra’s `ComponentSyncState` shows how to optimize later without blocking v0.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Full ECA replication | Matches Tundra wire | Huge spec + code | Too ambitious for v0 |
| CRDT scene graph | Offline-friendly | Complexity | Wrong problem for authoritative sim |
| Filesync assets | Simple | Not real-time | Separate asset ADR later |

## Consequences

**Positive**:
- Client can spawn Bevy entities from DTOs (ADR-003)
- Clear extension toward fine-grained sync

**Negative**:
- Large worlds = large snapshots (mitigation: ADR-012 interest + chunking)
- Duplicate data in memory on server and clients (mitigation: acceptable for early scale)

## Related

- [ADR-002](./002-sqlite-storage.md): Source schema for DTOs
- [ADR-009](./009-application-protocol-envelope-v0.md): Message kinds for snapshot/delta
- [ADR-010](./010-authoritative-avatar-state-v0.md): Dynamic avatars alongside static world
- [ADR-012](./012-interest-management-and-osm-tiles.md): What subset to send
- [ADR-013](./013-sqlite-migrations-and-server-writer.md): Sim reads/writes DB
