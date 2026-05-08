# ADR-012: Interest Management and OSM Tiles

---
**Metadata:**
- **ID**: ADR-012
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [interest, aoi, osm, networking]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-004, ADR-006, ADR-010, ADR-011]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Sending **all** regions/prims and **all** tile-related hints to every client wastes bandwidth. Classic Tundra’s [`SyncManager`](https://github.com/realXtend/tundra/blob/master/src/Core/TundraProtocolModule/SyncManager.h) supports **interest management**, an **observer** entity, **update periods**, and [`DefaultEntityPrioritizer`](https://github.com/realXtend/tundra/blob/master/src/Core/TundraProtocolModule/EntityPrioritizer.h) (distance-based priority). Observer position can be sent client→server ([`cObserverPositionMessage`](https://github.com/realXtend/tundra/blob/master/src/Core/TundraProtocolModule/TundraMessages.h)).

**Requirements**:
- **AOI** determines which **world DTOs** (ADR-011) and **avatar updates** (ADR-010) a client receives
- Policy **consistent** with OSM tile usage (ADR-004, ADR-006): same anchor math
- v0 **simple** (radius / region list) before full prioritization queues

## Decision

Implement **interest management v0** as: server tracks each client’s **observer position** (and optional orientation) in sim space; include only regions/prims within a **configurable radius** (and optionally always-include list). **OSM tiles** are **not** proxied through the sim in v0; clients derive `TileKey`s from ADR-006 using the **same** observer position they report (or server echo), keeping the server’s role to **world + avatars** only.

**Approach**:
- **Client → server**: Periodic **observer update** message (rate-limited), analogous in role to Tundra’s observer position message — not byte-compatible.
- **Server**: Filters snapshot/delta recipients; may subsample update frequency per client (inspired by Tundra `updatePeriod`).
- **Tiles**: Client continues HTTP fetch (ADR-004); optional future ADR for server-side tile cache or CDN redirect.

**Rust ecosystem**:
- Use **`glam`** for observer position and radius checks (`distance`, length) shared with ADR-006/010 DTOs.
- Rate limiting per connection: **`governor`** or a small in-house token bucket on top of **`tokio::time`**—prefer a **crate** over bespoke sleep logic for correctness under load.

## Rationale

**Primary Reasoning**:
1. **Scales world size**: Reduces replication fan-out.
2. **OSM alignment**: Tile choice follows the same spatial truth as AOI when coords are shared (ADR-006).
3. **Incremental**: Radius AOI is easy; Tundra-style prioritizer can plug in later.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| No AOI v0 | Fastest | Breaks at medium worlds | Small extra design now saves pain |
| Server proxies all map tiles | Single choke point | Bandwidth + ToU complexity | Defer |
| Grid-only AOI | Fast queries | Rough at region boundaries | Can combine with radius |

## Consequences

**Positive**:
- Clear knob for perf tuning (radius, rates)
- Keeps tile load on edge clients (ADR-004)

**Negative**:
- Desync if observer updates lag (mitigation: server extrapolate or clamp max speed)
- Edge cases at AOI boundary (mitigation: hysteresis / overlap regions)

## Related

- [ADR-006](./006-world-coordinate-and-osm-anchor.md): Shared space for AOI + tiles
- [ADR-004](./004-osm-tile-integration.md): Client tile fetch
- [ADR-010](./010-authoritative-avatar-state-v0.md): Observer may track camera/avatar anchor
- [ADR-011](./011-static-world-replication-v0.md): Filtered snapshot/delta
