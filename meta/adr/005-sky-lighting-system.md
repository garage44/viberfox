# ADR-005: Sky and Lighting System

---
**Metadata:**
- **ID**: ADR-005
- **Status**: Accepted
- **Date**: 2025-03-18
- **Tags**: [rendering, lighting, sky, atmosphere]
- **Decision Type**: technology_choice
- **Related**: [ADR-001, ADR-003]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: The scene needs a sky and proper lighting. A solid void background and basic directional light do not provide sufficient visual quality or a sense of place.

**Requirements**:
- Procedural sky visible from the camera
- Directional light (sun) aligned with sky appearance
- Physically plausible illuminance
- Compatible with Bevy 0.16

## Decision

Use `bevy_atmosphere` with the Nishita atmospheric model for procedural sky, and align the directional light with the atmosphere sun position.

**Approach**:
- `AtmospherePlugin` for procedural sky rendering
- `AtmosphereCamera` component on the main camera
- Nishita model with configurable `sun_position`
- Directional light transform aligned with `-sun_position` (Bevy illuminates along -Z)
- `AMBIENT_DAYLIGHT` (~110k lux) for sun illuminance
- Reduced ambient light (0.1 brightness) since sky contributes ambient

## Rationale

**Primary Reasoning**:
1. **bevy_atmosphere**: Mature, Bevy 0.16–compatible procedural sky crate
2. **Nishita**: Physically based atmospheric scattering model
3. **Aligned lighting**: Sun direction in sky matches directional light for visual consistency
4. **AMBIENT_DAYLIGHT**: Standard Bevy constant for outdoor daylight

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Skybox texture | Simple | Static, no time-of-day | Procedural is more flexible |
| Gradient sky | Lightweight | Unrealistic | Nishita looks better |
| Custom shader | Full control | High effort | bevy_atmosphere is sufficient |

## Consequences

**Positive**:
- Realistic sky with atmospheric scattering
- Sun and lighting visually consistent
- Extensible for future day/night cycle (update `sun_position` over time)
- No external assets required

**Negative**:
- Incompatible with WebGL (bevy_atmosphere uses compute shaders)
- Extra dependency and compile time

## Related

- [ADR-001](./001-bevy-game-engine.md): Bevy provides rendering and lighting
- [ADR-003](./003-ecs-architecture.md): Sky setup as startup system
- Systems: `src/main.rs` (`setup_sky`), `src/systems/free_camera.rs` (`AtmosphereCamera`)
