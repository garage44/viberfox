# ADR-022: OSM 3D Building Import (Overpass → Extruded Footprints)

---
**Metadata:**
- **ID**: ADR-022
- **Status**: Proposed
- **Date**: 2026-06-19
- **Tags**: [osm, buildings, rendering, import, dependency]
- **Decision Type**: architecture_pattern
- **Aligns with**: [G-04, P-03]
- **Roadmap tier**: P2 — map tier content
- **Doc resolution**: R1
- **Related**: [ADR-004, ADR-020, ADR-021]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: The map tier (ADR-020) should show OSM **3D buildings**, not just flat tiles. OSM stores buildings as 2D footprints (polygons) plus tags (`height`, `building:levels`, roof shape, etc.) under the *Simple 3D Buildings* schema — there are no ready-made meshes. We need to fetch that data and turn it into geometry, starting with the single existing region as a de-risking prototype.

**Requirements**:
- Pull building footprints + heights for a region's geographic bbox.
- Turn each footprint into a 3D mesh placed correctly relative to the map.
- Keep it client-side and additive (no protocol/sim changes for the prototype).

## Decision

Fetch buildings from the **Overpass API** for the region's tile bbox, parse the JSON, and **extrude each footprint** client-side into a mesh placed in region-local space.

**Approach** (as implemented in `systems/osm_buildings.rs`):
- **Fetch**: `way["building"](bbox); out geom;` to Overpass on a **background thread** (Overpass can take seconds; the window must not block).
- **Height**: `height` tag → else `building:levels × 3 m` → else 6 m default; tolerant of `"12 m"` strings.
- **Placement**: each vertex mapped via the same Web Mercator math as the tiles (fractional tile coords → the region quad).
- **Extrusion**: triangulated top cap (via **`earcutr`**, a pure-Rust ear-clipping triangulator — handles non-convex footprints) + vertical walls; wall normals oriented outward from the footprint centroid (OSM ring winding is not guaranteed).
- **Scale**: heights scaled by the quad-to-real-metre ratio so buildings sit proportionally on the displayed map (interim, until the real-metre frame of ADR-019 lands).

**New dependency**: `earcutr` (0.4) — small, pure Rust, no native build step.

## Rationale

1. **Overpass is the standard query path** for selective OSM data by bbox + tag (sibling to the tile fetch, ADR-004), reusing the existing `ureq` client.
2. **Client-side extrusion keeps the prototype additive** — no wire/sim changes; buildings are map-tier content.
3. **`earcutr`** is the minimal way to triangulate real (non-convex) footprints; alternatives (`geo` + heavier triangulation, or hand-rolled) add weight or bugs.
4. **Validated on real data** — Groningen buildings render aligned to the streets, retiring the "is OSM building data good enough" risk.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Cesium OSM Buildings / 3D Tiles | Prebuilt global LOD tilesets | Heavy spec; less control; external dependency | Revisit for planet-scale streaming, overkill now |
| Convex-only / fan triangulation | No dependency | Wrong for L-shaped/non-convex footprints | Most buildings are non-convex |
| Server-side extrusion | Thin client | Premature; needs protocol/sim work | Prototype is client-only |

## Consequences

**Positive**:
- Real 3D buildings on the map from live OSM data; loader + extruder carry forward to the streamed/planet pipeline unchanged.
- No protocol, sim, or storage changes.

**Negative**:
- **Overpass is public/rate-limited** — slow or occasionally failing (mitigation: background thread; failures log and skip; consider caching / a self-hosted endpoint later).
- **Interim scale hack** — heights scaled to the displayed quad, not true metres (mitigation: removed once ADR-019's real-metre frame lands).
- **v1 fidelity** — flat roofs, single colour, no roof shapes / `building:part` (mitigation: later iteration).

## Related

- [ADR-004](./004-osm-tile-integration.md): Tile fetch; buildings reuse the bbox/`ureq` path.
- [ADR-020](./020-map-tier-and-sim-tier-world-model.md): Buildings are map-tier content.
- [ADR-021](./021-region-tessellation-flat-clusters-quad-index.md): Will re-home buildings onto the real-metre region frame.
