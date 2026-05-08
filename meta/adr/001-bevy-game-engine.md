# ADR-001: Bevy as Game Engine

---
**Metadata:**
- **ID**: ADR-001
- **Status**: Accepted
- **Date**: 2025-03-18
- **Tags**: [technology, rendering, ecs]
- **Decision Type**: technology_choice
- **Related**: [ADR-003]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Need a 3D game engine for building a virtual world/metaverse-style application with regions, prims, and avatar movement.

**Requirements**:
- 3D rendering with PBR materials
- Entity-Component-System architecture for scalability
- Rust ecosystem (native performance, memory safety)
- Active development and community

## Decision

Use Bevy as the game engine for vibe-rs.

**Approach**: Bevy 0.16 with default plugins for rendering, ECS, windowing, and asset loading.

## Rationale

**Primary Reasoning**:
1. **Rust-native**: First-class Rust support, no FFI overhead
2. **ECS-first**: Built-in ECS aligns with data-oriented design for many entities
3. **Modular**: Plugin system allows adding only what's needed
4. **PBR rendering**: Built-in physically-based rendering for materials
5. **Active development**: Regular releases, growing ecosystem

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Amethyst | Mature ECS | Project inactive | Development halted |
| Godot | Mature, full-featured | GDScript/C++ not Rust | Rust bindings immature |
| Raw wgpu | Full control | Low-level, boilerplate | Too much work for scope |
| Three.js (WASM) | Web target | JS overhead, different runtime | Not native Rust |

## Consequences

**Positive**:
- Fast iteration with hot reload
- Clear separation of concerns via systems
- Good documentation and examples
- Built-in camera, lighting, transforms

**Negative**:
- Bevy API changes between versions (mitigation: pin version, migrate when needed)
- Smaller ecosystem than Unity/Unreal (mitigation: Bevy is sufficient for current scope)

## Related

- [ADR-003](./003-ecs-architecture.md): ECS architecture builds on Bevy's ECS
