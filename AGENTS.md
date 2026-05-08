# Instructions for AI assistants

This file explains **how architecture documentation works** in vibe-rs so agents stay aligned with **intent** (abstract), **priorities** (roadmap), **documentation depth** (resolution), **decisions** (ADRs), and **code**.

## Layers (what goes where)

| Layer | Location | Role |
|-------|----------|------|
| **Abstract** | `docs/meta/abstract/` | **Why** and **rules**: mission, goals (`G-*`), principles (`P-*`), **roadmap** (P0–P2), **ADR resolution** (R0–R3), capability map, glossary. Changes rarely. |
| **Decision log** | `docs/meta/adr/` | **What we chose** for a specific problem: context, decision, alternatives, consequences. Depth should match **resolution**; count of ADRs ≠ equal priority. |
| **Implementation** | `crates/` | Code must **respect** ADRs and **align** with `G-*` / `P-*` when work is architectural. |

**Abstract constrains and prioritizes; ADRs record choices; code implements.** Cite **`Aligns with: G-…, P-…`** in ADRs instead of pasting the full mission.

## Read order (before large or cross-cutting work)

1. [`docs/meta/abstract/vision.md`](docs/meta/abstract/vision.md) — `G-*` goals, non-goals
2. [`docs/meta/abstract/principles.md`](docs/meta/abstract/principles.md) — `P-*` principles
3. [`docs/meta/abstract/roadmap.md`](docs/meta/abstract/roadmap.md) — **P0 / P1 / P2** and which ADRs matter first
4. [`docs/meta/abstract/adr-resolution.md`](docs/meta/abstract/adr-resolution.md) — **R0–R3** (how thin or deep an ADR should be)
5. [`docs/meta/abstract/capability-map.md`](docs/meta/abstract/capability-map.md) — themes, priority, ADR links
6. [`docs/meta/adr/index.md`](docs/meta/adr/index.md) — full ADR list
7. Relevant ADR files and then the crates they mention

Full discipline (when an ADR is required, definition of done): [`docs/meta/abstract/README.md`](docs/meta/abstract/README.md).

## Identifiers

- **`G-01` … `G-08`** — Outcome goals in `abstract/vision.md`
- **`P-01` … `P-07`** — Durable rules in `abstract/principles.md`
- **P0 / P1 / P2** — Roadmap tiers in `abstract/roadmap.md`
- **R0–R3** — Doc depth in `abstract/adr-resolution.md`
- **`ADR-NNN`** — Point-in-time decisions under `docs/meta/adr/`

## When to add or update an ADR

**Create or supersede an ADR** when changing wire protocol, authority, persistence schema, security boundaries, major dependencies, or sim/client/process boundaries.

**Skip a new ADR** for bug fixes, refactors, or performance work **inside** an existing ADR’s scope, and for routine dependency bumps that do not change architecture.

**Depth:** match **[adr-resolution.md](docs/meta/abstract/adr-resolution.md)** — do not default every topic to a full R2 ADR. **Priority:** follow **[roadmap.md](docs/meta/abstract/roadmap.md)** — P2 topics can stay thin until promoted.

When in doubt, prefer a **short ADR** over silent drift. See the trigger table in [`abstract/README.md`](docs/meta/abstract/README.md).

## New ADR checklist

1. Copy [`docs/meta/adr/template.md`](docs/meta/adr/template.md).
2. Set **Aligns with** to relevant `G-*` and `P-*`, or **`N/A`** plus one line (e.g. refactor under ADR-010).
3. Optionally set **Roadmap tier** and **Doc resolution** (see template).
4. Add a row to [`docs/meta/adr/index.md`](docs/meta/adr/index.md) in the **same change set** as the new file.
5. If capability status changes meaningfully, update [`docs/meta/abstract/capability-map.md`](docs/meta/abstract/capability-map.md); if **tier** changes, update [`docs/meta/abstract/roadmap.md`](docs/meta/abstract/roadmap.md).

## Cursor rules

- **ADRs + abstract**: `.cursor/rules/adr.mdc` — consult when making architectural proposals.
- **Build scope**: `.cursor/rules/build-and-iteration.mdc` — prefer scoped `cargo check -p …` over full workspace builds when possible.

## Human-oriented docs

- Project overview and run instructions: [`README.md`](README.md).
- Meta entry and links: [`docs/meta/README.md`](docs/meta/README.md).
