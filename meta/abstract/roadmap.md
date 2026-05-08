# Roadmap (priorities and ADR sequencing)

This file ranks **capabilities** and ties them to **documentation depth** (see [adr-resolution.md](adr-resolution.md)). **ADR numbers are not pre-allocated here** — you add an ADR when a tier + resolution calls for a recorded decision, then link it from [capability-map.md](capability-map.md).

## Priority tiers

| Tier | Meaning |
|------|---------|
| **P0** | **Must** land for a credible **shared, authored, visitable** world with **safe edits** |
| **P1** | **Important** soon: operations, safety for multi-user, **accelerated authoring** (incl. LLM) once shape is known |
| **P2** | **Valuable** but **after** core loop; stay at **R0** or thin **R1** ADRs until promoted |

## P0 — core loop

Capabilities that already have decision records list **ADR** ids. Others say **when to add ADRs** (tier + target resolution); create **new numbered ADRs** only when you start that work.

| Capability | Recorded in ADRs | When adding ADRs |
|------------|------------------|------------------|
| World coordinates & replication | 006, 011, 012 | Extend or supersede as implementation evolves (**R2+**) |
| Transport & protocol envelope | 008, 009 | Same |
| Authoritative avatar | 010 | Same |
| Persistence & migrations | 002, 013 | Same |
| Sim vs client, workspace | 007, 015 | Same |
| **Asset storage & delivery** | *none* | **Before** general mesh/texture authoring: add **R2** (or **R3** if split) ADR(s) for identity, fetch path, upload + integrity |
| **Authentication** | 016 | Extend **016** when adding OAuth/OIDC, MFA, or federation; **R2+** as features harden |
| **Authorization / world edits** | 016 (implicit v0) | **R2** ADR for **roles/capabilities** when the mutating surface needs more than one trust level; until then ADR-016’s “authenticated = full v0 mutator” applies |

## P1 — scale, safety, fast iteration

| Capability | Recorded in ADRs | When adding ADRs |
|------------|------------------|------------------|
| Runtime config & ops | 014 | Extend as needed |
| **Rate limits & quotas** | *none* | **Before** wide exposure: **R2** ADR |
| **Observability & abuse signals** | *none* | **Alongside** limits: **R2** ADR |
| **LLM-constrained world ops** | *none* | Start with **R1** boundary ADR after spike; add **R2+** child ADRs when **action schema**, **model boundary**, and **review UX** are defined |
| **Scripting / integrations** | *none* | **R1** until demand is clear; **R2** when embedding and API surface stabilize |

## P2 — social & world extensions

| Capability | Recorded in ADRs | When adding ADRs |
|------------|------------------|------------------|
| Text chat | *none* | **R1** when P0 social need is explicit; **R2** if protocol and abuse story grow |
| WebRTC / media | *none* | **R1** roadmap-only until auth + observability basics exist; then dedicated ADR(s) |
| Companion / GNSS | *none* | **R1** first; **R2** after privacy/threat model |
| Rendering stack | 001, 003, 004, 005 | Evolve with accepted pattern |

## Sequencing hints

1. Do **not** block **assets** on **chat** or **WebRTC**.
2. Prefer **authentication → authorization** before **asset upload** or **chat send** paths.
3. **LLM**: spike **structured ops + validation** before a large single ADR; **split** by concern when promoting to **R2+**.
4. When a theme moves **P2 → P1** (or **P1 → P0**), update this file and [capability-map.md](capability-map.md), then add or deepen ADRs per [adr-resolution.md](adr-resolution.md).

## Related

- [vision.md](vision.md) — `G-*` goals
- [adr-resolution.md](adr-resolution.md) — R0–R3 and **workflow**
- [capability-map.md](capability-map.md) — link new ADRs here
- [../adr/index.md](../adr/index.md) — decision log
