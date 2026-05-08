# ADR-010: Authoritative Avatar State (v0)

---
**Metadata:**
- **ID**: ADR-010
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [avatar, networking, authority, simulation]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-003, ADR-006, ADR-009, ADR-012]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: In a multi-user world, **avatar transform** (position, orientation) must have a single **source of truth** or clients will disagree. Classic Tundra sends **Entity Actions** for client-driven behavior ([`cEntityActionMessage`](https://github.com/realXtend/tundra/blob/master/src/Core/TundraProtocolModule/TundraMessages.h)) interpreted on the server.

**Requirements**:
- Server **owns** pose at a defined tick rate
- Client sends **intent** (input), not authoritative teleport (except debug tools, gated)
- Same coordinate frame as ADR-006
- Explicit **non-goals** for v0 to avoid scope creep

## Decision

The **simulation** integrates **movement intent** each tick and **broadcasts** authoritative transform updates (or includes them in bundled sync). Clients **never** set final pose for networked avatars without server confirmation.

**Approach**:
- **Intent message** (ADR-009 kind): e.g. walk vector, fly flag, jump — compact bitfield or small struct; rate-limited server-side.
- **Tick rate**: Fixed sim step (e.g. 20–60 Hz) documented in config (ADR-014); networking may batch outbound updates.
- **State on server**: Stored in sim world model (ECS or structs); replicated per ADR-011/012.
- **Out of scope v0**: Full rigid-body physics sync, animation skeleton replication, vehicle controllers.

**Conceptual analogue**: Tundra **Entity Action** as RPC; v0 uses one dedicated intent opcode instead of a general action system.

**Rust ecosystem**:
- Represent intent and replicated pose with **`serde`**-serializable structs in `vibe_core`, using **`glam`** (`Vec3`, `Quat`) with `serde` features aligned with Bevy 0.16 (ADR-003).
- Optional **`bitflags`** for compact input masks (walk/fly/jump) instead of raw integers.
- Sim tick scheduling: **`tokio::time::interval`** or a dedicated **`std::time`** loop in v0; integrate with **`tracing`** spans per tick when debugging.

## Rationale

**Primary Reasoning**:
1. **Anti-cheat / consistency**: Standard for virtual worlds (OpenSim/Tundra-style).
2. **Matches ECS client**: Client-side `Avatar` still exists for rendering; network layer overwrites from server state (ADR-003).
3. **Incremental**: Intent can start as coarse velocity before full input map.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Client-authoritative pose | Low latency | Trivial exploit | Unacceptable for shared world |
| Lockstep / input history | Fair FPS | Complex | Overkill for v0 |
| Full physics on server | Realistic | Cost + ADR scope | Deferred |

## Consequences

**Positive**:
- Clear contract for ADR-011 (what to include in snapshots/deltas)
- AOI (ADR-012) can key off avatar positions

**Negative**:
- Perceived latency without client prediction (mitigation: optional prediction ADR later)
- Server CPU for all avatars (mitigation: low tick + simple kinematics)

## Related

- [ADR-006](./006-world-coordinate-and-osm-anchor.md): Pose in sim space
- [ADR-009](./009-application-protocol-envelope-v0.md): Intent message kinds
- [ADR-012](./012-interest-management-and-osm-tiles.md): Who receives pose updates
- [ADR-003](./003-ecs-architecture.md): Client ECS applies replicated state
