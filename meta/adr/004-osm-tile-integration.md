# ADR-004: OSM Tile Integration for Region Textures

---
**Metadata:**
- **ID**: ADR-004
- **Status**: Accepted
- **Date**: 2025-03-18
- **Tags**: [rendering, tiles, osm]
- **Decision Type**: technology_choice
- **Related**: [ADR-001, ADR-003]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Regions need textured ground planes. Use real-world map imagery for geographic context.

**Requirements**:
- Fetch map tiles by (x, y, z) coordinates
- Load asynchronously
- Cache for performance
- Integrate with Bevy's asset system

## Decision

Use OpenStreetMap tiles via HTTP (ureq) with a `TileCache` resource and `TileLoader` system.

**Approach**:
- `TileKey(x, y, z)` for tile lookup
- `TileCache` for handles and loading state
- `RegionTile` component on region entities
- `load_region_tiles` and `update_region_materials` systems
- `bevy_image` for image loading from bytes

## Rationale

**Primary Reasoning**:
1. **OSM**: Free, legal to use tiles
2. **ureq**: Simple HTTP, no async runtime
3. **TileCache**: Avoid redundant fetches
4. **LOD**: `lod_level` for future LOD (0=2x2, 1=1x1, 2=1x1 low-res)

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| reqwest | Async | Extra deps | ureq is simpler |
| Pre-bake tiles | Faster load | Large assets | Dynamic tiles preferred |
| Mapbox | Better quality | Cost, API key | OSM is free |

## Consequences

**Positive**:
- Realistic ground textures
- Caching reduces network traffic
- Standard tile format (z/x/y)

**Negative**:
- Network dependency (mitigation: cache, fallback)
- OSM ToU: respect usage policy (mitigation: reasonable rate, attribution)
- Blocking HTTP in system (mitigation: acceptable for single-user)

## Related

- [ADR-001](./001-bevy-game-engine.md): Bevy provides rendering
- [ADR-003](./003-ecs-architecture.md): Tile loading as system
- Systems: `src/systems/tile_loader.rs`, `src/systems/rendering.rs`
