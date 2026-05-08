# ADR-014: Runtime Configuration and Operations

---
**Metadata:**
- **ID**: ADR-014
- **Status**: Proposed
- **Date**: 2025-03-21
- **Tags**: [config, ops, deployment]
- **Decision Type**: architecture_pattern
- **Related**: [ADR-007, ADR-008, ADR-012, ADR-013, ADR-016]
- **Supersedes**: []
- **Superseded By**: []
---

## Context

**Problem**: Running `vibe_sim` on a host (ADR-007) requires **bind address**, **port**, **database path**, **tile URL template** (for operator documentation even if clients fetch tiles), **log level**, and **AOI parameters** (ADR-012) without recompiling. Tundra historically uses **CLI flags** and **JSON config** (`tundra.json`).

**Requirements**:
- Sensible defaults for local dev
- **Overrides** via env vars and/or config file
- Documented for minimal VPS deployment

## Decision

Use a **layered config**: defaults in code → optional config file → **environment variable** overrides for secrets and container deploys.

**Rust ecosystem**:
- **`figment`** with **`Toml`** + **`Env`** providers and a `serde`-derived settings struct—single pattern for file + `VIBE_*` (or `APP_*`) env overrides without ad hoc `std::env::var` scattered everywhere.
- CLI overrides: **`clap`** (derive) mirroring the same struct fields where useful.
- Logging: **`tracing`** + **`tracing-subscriber`** (`fmt`, **`EnvFilter`**); enable the **`json`** feature (or crate **`tracing-bunyan-formatter`**) when structured logs for aggregators are needed.

**Approach** (minimum keys):
- **Network**: `listen_host`, `listen_port` (ADR-008)
- **Data**: `database_path` (ADR-013)
- **Tiles**: `osm_tile_url_template` or equivalent (ADR-004) for docs/client handoff
- **Sim**: `tick_hz`, **AOI radius** defaults (ADR-012)
- **Logging**: level + optional structured output

Default file path: **`vibe.toml`** (loaded through **`figment`**’s `Toml` provider). CLI flags may mirror the same keys via **`clap`** (implementation detail).

## Rationale

**Primary Reasoning**:
1. **12-factor** style env overrides are standard for containers.
2. **File config** matches Tundra/OpenSim operator expectations.
3. **Separation**: Keeps secrets out of source control.

**Alternatives Considered**:

| Alternative | Pros | Cons | Rejected Because |
|------------|------|------|------------------|
| Env only | Simple | Tedious for many keys | File + env is nicer |
| CLI only | Explicit | Poor for services | Not sufficient alone |
| Dynamic remote config | Fancy | Ops burden | Future |

## Consequences

**Positive**:
- Repeatable deploys; easy to document “minimum viable server”
- Ties ADR-007–013 together operationally

**Negative**:
- Config schema must be versioned with protocol (mitigation: document breaking changes in CHANGELOG)

## Related

- [ADR-007](./007-simulation-vs-client-process-model.md): Sim binary reads config
- [ADR-008](./008-network-transport-layer.md): Listen address/port
- [ADR-012](./012-interest-management-and-osm-tiles.md): AOI defaults
- [ADR-013](./013-sqlite-migrations-and-server-writer.md): DB path
