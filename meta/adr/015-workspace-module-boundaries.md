# ADR-015: Workspace Module and Crate Boundaries

---
**Metadata:**
- **ID**: ADR-015
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [workspace, crates, architecture]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-003, ADR-007, ADR-008, ADR-009]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Networking (ADR-008/009), coordinates (ADR-006), and protocol types will **outgrow** a single `main.rs` + flat `src/`. Without boundaries, sim and client **duplicate** logic or create import cycles. Classic Tundra separates **core scene** from **protocol module** ([`TundraProtocolModule`](https://github.com/realXtend/tundra/tree/master/src/Core/TundraProtocolModule)) from **client shell**.

**Requirements**:
- **`vibe_core`**: shared types, protocol, coordinate math — **no** Bevy dependency (or optional `bevy` feature gated)
- **`vibe_sim`**: I/O, DB, tick loop — may depend on `vibe_core` + async/runtime
- **`vibe_client`**: Bevy app — depends on `vibe_core`
- Clear rule: **where new message types live**

## Decision

Restructure the Cargo workspace into at least **`vibe_core`**, **`vibe_sim`**, and **`vibe_client`** (names may be prefixed with package name used in `Cargo.toml`), with **strict dependency direction**: `vibe_client` → `vibe_core`, `vibe_sim` → `vibe_core`, **no** `vibe_sim` ↔ `vibe_client` dependency.

**Approach**:
- **Protocol enums / DTOs**: `vibe_core::protocol` (or similar module path).
- **Coordinate + tile key helpers**: `vibe_core::world` (ADR-006).
- **Bevy systems** stay in `vibe_client` only; sim may use a minimal ECS later but must not force Bevy on `vibe_core`.
- **Future**: Optional `vibe_proto` crate if generated code appears — only if duplication hurts (YAGNI for v0).

**Rust ecosystem**:
- Root **`[workspace]`** with members for `vibe_core`, `vibe_sim`, `vibe_client` (exact names per `Cargo.toml`); use **`[workspace.dependencies]`** for aligned versions of **`tokio`**, **`serde`**, **`tracing`**, **`thiserror`**.
- Integration tests for **`vibe_sim`**: prefer **`cargo-nextest`** in CI when the suite grows.
- Hot paths (codec/framing): optional **`criterion`** benches in `vibe_core` once there is measurable code.

**Per-component `sync` flags** (Tundra `SceneDesc` pattern): if field-level replication returns, those metadata bits live beside component definitions in `vibe_core`, not scattered in UI code.

## Rationale

**Primary Reasoning**:
1. **Compile times**: Smaller crates rebuild faster during client-only iteration.
2. **Correct layering**: Prevents accidental renderer imports in the sim.
3. **Matches ADR-007**: Two binaries naturally map to two crate roots + shared lib.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Monorepo single crate | Simplest now | Becomes tangled | Will hurt at ADR-008 |
| Many micro-crates | Max isolation | Overhead | Too many for current size |
| Feature flags only | One package | Still couples deps | Weak boundary vs crates |

## Consequences

**Positive**:
- Enforces ADR-008/009 ownership (codec in core)
- Easier testing of protocol without GPU

**Negative**:
- Initial refactor cost (mitigation: incremental move starting with types)
- Public API on `vibe_core` needs semver care (mitigation: keep internal until stable)

## Related

- [ADR-007](./007-simulation-vs-client-process-model.md): Binaries map to crates
- [ADR-008](./008-network-transport-layer.md): Transport impl likely `vibe_sim` (or `vibe_core` if shared with test harness)
- [ADR-009](./009-application-protocol-envelope-v0.md): Envelope types in `vibe_core`
- [ADR-003](./003-ecs-architecture.md): ECS remains client-focused unless sim adopts Bevy later
