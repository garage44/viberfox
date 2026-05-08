# ADR-009: Application Protocol Envelope (v0)

---
**Metadata:**
- **ID**: ADR-009
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [protocol, networking, api]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-008, ADR-010, ADR-011, ADR-016]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Framed TCP (ADR-008) needs a **versioned, extensible** inner format so client and sim can evolve. Classic Tundra defines a large **numeric opcode** space in [TundraMessages.h](https://github.com/realXtend/tundra/blob/master/src/Core/TundraProtocolModule/TundraMessages.h) (login 100/101, scene sync 109+, entity action 120, …) and expects **login before scene traffic**.

**Requirements**:
- **Protocol version** negotiation or rejection
- **Message kind** (discriminant) + **correlation** where needed
- **Session gate**: no world snapshot before successful handshake
- **Small v0 surface**: not a full Entity–Component–Attribute schema

## Decision

Define a **binary envelope** for every payload: `protocol_version`, `message_kind` (`u16` or `u32`), optional `request_id`, then **length-prefixed payload** decoded per kind. Implement **Handshake → Ack** (or Error) before **WorldSnapshot** or **Intent** messages.

**Approach**:
- **Handshake**: client sends supported version + client id token (opaque string for v0); server replies Ack + assigned session parameters or Error with code.
- **Serialization**: Use **`serde`** for all message bodies; encode on the wire with **`postcard`** (compact, `no_std`-friendly, deterministic enough for tests) unless profiling favors **`bincode`** 2—pick **one** codec for v0 and document the crate version. Optional JSON **only** for debug logging, not on hot paths.
- **Opcode naming**: Rust **`enum` with `#[repr(u16)]` or `#[repr(u32)]`** (or explicit discriminant field) for `message_kind`; optional **`strum::Display`** for human-readable logs without hand-maintained string tables.
- **Session tokens**: Prefer **`uuid::Uuid`** (v4 or v7 per policy) for opaque session ids rather than ad hoc strings.

**Rust ecosystem summary**: `serde` + `postcard` (or `bincode`) + `uuid` + `thiserror` for typed protocol errors in `vibe_core`.

**Ordering** (aligned with Tundra login-before-scene): Handshake → (optional lightweight Ready) → WorldSnapshot / deltas / Intent.

## Rationale

**Primary Reasoning**:
1. **Evolution**: Version + kind allow adding messages without new transports.
2. **Debugging**: `request_id` links errors to client actions.
3. **Familiar flow**: Same *gate* as Tundra/FIWARE sync docs without copying their byte layout.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| JSON-only messages | Human-readable | Bandwidth + speed | Poor fit for snapshots |
| Protobuf / **tonic** gRPC | Schema tooling, multi-lang | Heavier codegen, HTTP/2 stack | Defer until multi-language clients |
| Clone Tundra opcodes 1:1 | Interop dream | Wrong security & layout | Explicit non-goal |

## Consequences

**Positive**:
- Clear extension point for ADR-010/011 message kinds
- Testable with golden vectors per message kind

**Negative**:
- Every new kind needs versioning discipline (mitigation: feature flags in handshake)
- Manual doc must stay in sync with Rust enums (mitigation: single `vibe_core` module; optional **`serde_json`** one-liners in tests for golden vectors)

## Related

- [ADR-008](./008-network-transport-layer.md): Frames carry this envelope
- [ADR-010](./010-authoritative-avatar-state-v0.md): Intent message kinds
- [ADR-011](./011-static-world-replication-v0.md): Snapshot / delta kinds
- [Tundra protocol wiki](https://github.com/realXtend/tundra/wiki/Tundra-protocol) — byte-level reference we do **not** implement
