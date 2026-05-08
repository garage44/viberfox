# ADR-007: Simulation vs Client Process Model

---
**Metadata:**
- **ID**: ADR-007
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [server, client, headless, architecture]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-003, ADR-008, ADR-013, ADR-015]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: The app is today a **single Bevy process** with local input and SQLite. A virtual world **server** needs an **authoritative** loop (time step, world mutations) separate from **presentation** (rendering, local prediction optional).

**Requirements**:
- Clear ownership: which process reads/writes SQLite, which accepts network clients
- Headless operation for hosting (no GPU)
- Room to grow without microservices or duplicate business logic

## Decision

Split into **simulation** (headless-capable, authoritative state, DB writer) and **client** (Bevy render + input + network consumer) as **separate binaries** sharing a **`vibe_core`** library.

**Approach**:
- **`vibe_sim`**: Runs fixed-tick (or fixed-update) loop; loads config (ADR-014); owns network listen; applies intents; persists via ADR-013 rules.
- **`vibe_client`** (current app evolution): Connects to sim; spawns entities from snapshots; sends intent; may predict locally later (out of scope for this ADR).
- **Shared `vibe_core`**: Types, coordinate model (ADR-006), protocol codec (ADR-008/009), DTOs for regions/prims.

**Rust ecosystem**:
- **`tracing`** (+ **`tracing-subscriber`** in each binary, ADR-014) so sim and client share the same structured log idioms (`info!`, spans, connection/session fields).
- Run **`vibe_sim`** on the **`tokio`** runtime (ADR-008) even if the first sim loop is mostly sync + `block_on` for rusqlite—avoids rewriting when networking goes fully async.

**Reference**: Classic Tundra runs a **server** mode headless with scene load; same *separation of concerns*, not the same stack (see [realXtend/tundra](https://github.com/realXtend/tundra) docs and `TundraConsole` usage).

## Rationale

**Primary Reasoning**:
1. **Authority boundary**: Matches how OpenSim-style regions and Tundra-style sync think about “sim vs viewer.”
2. **Deployment**: VPS can run `vibe_sim` only (ADR-014).
3. **Testing**: Sim binary can run integration tests without a window.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| One binary + `--server` flag | Fewer crates | Tangles features | Becomes unmaintainable as net code grows |
| Microservices | Scale story | Ops complexity | Out of scope for gradual build |
| Client-authoritative | Easiest multiplayer hack | Cheating, divergence | Virtual world expectation is server authority |

## Consequences

**Positive**:
- Forced clean API between sim and render paths
- Aligns with ADR-008 (transport owned by sim) and ADR-013 (single DB writer)

**Negative**:
- Two binaries to build and version together (mitigation: workspace crate, shared semver)
- Refactor cost from monolithic `main.rs` (mitigation: ADR-015 boundaries)

## Related

- [ADR-003](./003-ecs-architecture.md): Client keeps Bevy ECS; sim may use ECS or minimal state (implementation choice after this ADR)
- [ADR-008](./008-network-transport-layer.md): Sim accepts connections
- [ADR-013](./013-sqlite-migrations-and-server-writer.md): Only sim opens DB for writes
- [ADR-015](./015-workspace-module-boundaries.md): Crate layout for `vibe_core` / `vibe_sim` / `vibe_client`
