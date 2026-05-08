# ADR-008: Network Transport Layer (v0)

---
**Metadata:**
- **ID**: ADR-008
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [networking, transport, server]
- **Decision Type**: technology_choice
- **Related**: [ADR-007, ADR-009]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Client and simulation processes (ADR-007) need a **reliable, debuggable** byte pipe for application messages (ADR-009). Classic Tundra uses **kNet** (UDP default, TCP optional) via Kristalli; Tundra-Urho3D adds **WebSocket** binary frames for web clients.

**Requirements**:
- Works **headless** on server
- **Framed** messages (length-delimited or stream-safe)
- Rust ecosystem fit; no Qt/kNet dependency
- Room to add **TLS** or **WebSocket** front-end later without rewriting app messages

## Decision

Use **TCP** with **length-prefixed frames** (e.g. `u32` BE/LE fixed in spec + payload) as the **v0 transport** between `vibe_sim` and `vibe_client`.

**Approach**:
- One connection per client session; sim is listener.
- Frame = `length || payload`; payload interpreted by ADR-009 envelope.
- Document max frame size to bound memory.
- **Later**: Optional QUIC or WebSocket **terminator** in front of the same logical messages (not v0).

**Rust ecosystem (preferred stack)**:
- **`tokio`** (`net`, `io-util`, `macros`, `rt-multi-thread`) for async accept/read/write on the sim and client network tasks.
- **`tokio-util::codec::LengthDelimitedCodec`** (configure **max frame length** and length field width/endianness to match the written spec) for **battle-tested framing** instead of hand-rolled `read_exact` loops.
- **`bytes::BytesMut`** (pulled in via tokio/tokio-util) for zero-copy-ish buffer reuse where it matters.
- Errors: **`thiserror`** in `vibe_core` / library boundaries; **`anyhow`** (or `eyre`) only at **binary** `main` for top-level reporting—standard Rust layering.
- Future **TLS** on TCP: **`tokio-rustls`** + **`rustls`** (ring/aws-lc per platform policy), not OpenSSL via FFI unless required.

**Comparison (not implementation)**: [Kristalli / kNet](https://github.com/realXtend/tundra) chooses UDP/TCP at the socket layer with its own reliability; we trade that for **simple TCP** first, accepting latency vs classic Tundra defaults.

## Rationale

**Primary Reasoning**:
1. **Reliability**: World snapshots and ordered setup match TCP semantics for v0.
2. **Simplicity**: No custom ACK layer in the first iteration.
3. **Framing**: Same *spirit* as discrete messages in [TundraMessages.h](https://github.com/realXtend/tundra/blob/master/src/Core/TundraProtocolModule/TundraMessages.h) (opcode lives inside payload per ADR-009).

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| UDP + custom reliability | Lower latency | More code | Defer until profiling says TCP is bottleneck |
| QUIC (**`quinn`**) | Modern, built on **tokio** | Heavier dep for v0 | Revisit when TLS + NAT traversal matter |
| WebSocket v0 (**`tokio-tungstenite`**) | Browser-friendly | Extra framing + deps in sim | Defer to dedicated gateway ADR |

## Consequences

**Positive**:
- Easy to inspect with tcpdump / simple test harness
- Aligns with the mainstream **async Rust** stack (**tokio** + **tokio-util** codecs)

**Negative**:
- Head-of-line blocking under packet loss (mitigation: acceptable for v0; measure later)
- No browser client without a bridge (mitigation: future WebSocket adapter)

## Related

- [ADR-007](./007-simulation-vs-client-process-model.md): Sim owns the listener
- [ADR-009](./009-application-protocol-envelope-v0.md): Payload format inside frames
- Tundra-Urho3D [User and Programmer's Guide](https://github.com/realXtend/tundra-urho3d/blob/master/doc/User_and_Programmers_guide.md) — WebSocket binary framing as contrast
