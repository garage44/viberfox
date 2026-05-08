# Vibe-RS

A Bevy-based 3D virtual world application.

**AI assistants:** see [AGENTS.md](AGENTS.md) for **`docs/meta/`** (abstract + ADRs), roadmap tiers, ADR resolution, and the **meta → ADR → code** workflow.

## Features

- **Region Storage**: SQLite database for storing regions with geographic coordinates
- **Prim Storage**: SQLite database for storing 3D primitives (boxes, spheres, cylinders, cones, toruses)
- **Avatar Movement**: Walk and fly modes; third-person **camera-relative** WASD (W/S forward–back in view, A/D strafe)
- **Camera System**: Third-person camera following the avatar

## Controls

- **W / ↑**: Move forward (into the screen, relative to the orbit camera)
- **S / ↓**: Move backward (retreat; fox stays facing into the camera, no 180° flip)
- **A,D / ←,→**: Strafe left/right on the ground (relative to the camera)
- **Left-drag**: Orbit the camera around the avatar (movement stays camera-relative)
- **Space**: Fly up (when in fly mode)
- **Shift**: Fly down (when in fly mode)
- **F**: Toggle fly/walk mode

## Development

### Prerequisites

- Rust and Cargo installed (see [rustup.rs](https://rustup.rs/))

**For Fedora/Distrobox users:**
```bash
# Run the setup script (installs all required dependencies)
./setup-fedora.sh
```

### Running in Development Mode

This repo is a **workspace** (`vibe_core`, `vibers-sim`, `vibers-rs`). The game client is `vibers-rs`:

```bash
cargo run -p vibers-rs
```

**Assets:** the client loads from **`assets/`** at the workspace root (e.g. `models/animated/Fox.glb` for the avatar). See [`assets/README.md`](assets/README.md).

**Compile-time tuning (root `Cargo.toml`):** this repo follows [Bevy’s setup guide](https://bevy.org/learn/quick-start/getting-started/setup/) with a **compile-first default** and an optional **playable-debug** profile.

- **Bevy feature set:** slimmer than upstream `default` (3D, glTF, animation, windowing only)—less to type-check per rebuild. Re-enable features in the workspace `bevy` entry if you add UI, audio, picking, etc.
- **Default `dev` profile:** workspace crates at **`opt-level = 1`** (aligned with [Compile with performance optimizations](https://bevy.org/learn/quick-start/getting-started/setup/#compile-with-performance-optimizations)); **dependencies at `opt-level = 0`** so Bevy/wgpu rebuild faster than the book’s `opt-level = 3` on `*`.
- **`split-debuginfo = "unpacked"`** (Linux): cheaper incremental **links** after large compiles.

**Smoother framerate in debug** (if the client feels sluggish — trades much slower dependency rebuilds):

```bash
cargo run -p vibers-rs --profile dev-bevy
```

`dev-bevy` matches the book’s **`[profile.dev.package."*"] opt-level = 3`** (with `vibe_core` left at `0`). See the same [Bevy setup § Compile with performance optimizations](https://bevy.org/learn/quick-start/getting-started/setup/#compile-with-performance-optimizations).

**Faster iterative compiles / links** ([Enable fast compiles](https://bevy.org/learn/quick-start/getting-started/setup/#enable-fast-compiles-optional)):

```bash
cargo run -p vibers-rs --features fast-dev
```

`fast-dev` turns on Bevy’s `dynamic_linking`. Combine with `--profile dev-bevy` if you want both. Do not ship release builds with `fast-dev`.

**Linker:** `.cargo/config.toml` uses **clang + lld** as in the book. **mold** is often faster—see [Alternative linkers](https://bevy.org/learn/quick-start/getting-started/setup/#alternative-linkers) and comments in `.cargo/config.toml`.

**Scoped work:**
- First full client build still takes a while; afterward, small edits rebuild a subset.
- Use `cargo check -p vibers-sim` / `cargo run -p vibers-sim` when you are not touching the client (no Bevy).

### Server (`vibers-sim`) config (ADR-013, ADR-014)

- Optional **`vibe.toml`** in the working directory: keys `listen`, `database_path`, `tick_hz`, `aoi_radius`, `osm_tile_url_template` (use `{z}`, `{x}`, `{y}` placeholders; default is openstreetmap.org).
- **Environment:** same keys with prefix `VIBE_` (e.g. `VIBE_listen`, `VIBE_osm_tile_url_template`).
- **CLI overrides:** `vibers-sim --listen 0.0.0.0:4747 --database-path ./data/regions.db --tick-hz 30 --aoi-radius 800 --osm-tile-url-template 'https://…/{z}/{x}/{y}.png'`
- The server sends **`osm_tile_url_template`** in the handshake so online clients use the same tile source (ADR-014).
- **Before schema upgrades:** copy the SQLite file (ADR-013); migrations run automatically on sim startup.

**Wire format (ADR-008–009):** TCP length-delimited frames, little-endian length; each frame body is an **app frame** (`protocol_version` + `message_kind` + `request_id` + postcard payload). `PROTOCOL_VERSION` is **4** in `vibe_core` (client intent includes `display_yaw`).

This will:
1. Compile the project in debug mode
2. Initialize the SQLite database at `data/regions.db` if it doesn't exist
3. Load regions and prims from the database
4. Render regions as planes and prims as 3D shapes
5. Spawn an avatar that you can control

### Building for Release

```bash
cargo build -p vibers-rs --release
```

Binary: `target/release/vibers-rs` (do **not** use `--features fast-dev` for release).

### Development Workflow

1. **Make changes** under `crates/vibers-rs/src/` (or other workspace crates)
2. **Run** `cargo run -p vibers-rs` to test changes
3. **Check for errors** - Rust's compiler will catch type errors and many logic errors at compile time
4. **Iterate** - Bevy's ECS architecture makes it easy to add new systems and components

### Adding New Features

The project uses Bevy's Entity Component System (ECS) architecture:

- **Components** (`crates/vibers-rs/src/components.rs`): Data attached to entities
- **Resources** (`crates/vibers-rs/src/resources.rs`): Global state
- **Systems** (`crates/vibers-rs/src/systems/`): Logic that operates on components and resources

To add a new feature:
1. Define components/resources in their respective files
2. Create a system under `crates/vibers-rs/src/systems/`
3. Register the system in `crates/vibers-rs/src/main.rs` using `.add_systems()`

### Architecture

**Architecture docs** live under [docs/meta/](docs/meta/): **abstract** (vision, roadmap, ADR resolution) and **adr** (decision log). See [docs/meta/README.md](docs/meta/README.md) for read order; [ADR index](docs/meta/adr/index.md) lists decisions.

### Project Structure

```
vibe-rs/
├── Cargo.toml              # Workspace root (shared dev profiles)
├── crates/
│   ├── vibe_core/src/      # Shared protocol + OSM/tile types
│   ├── vibers-sim/         # Headless server binary
│   └── vibers-rs/src/      # Bevy client
│       ├── main.rs
│       ├── components.rs
│       ├── resources.rs
│       ├── systems/
│       └── db/
└── data/regions.db         # Created on first local run (client or sim)
```

### Key Files

- **`crates/vibers-rs/src/main.rs`**: Bevy app, systems, `--connect` for online mode
- **`crates/vibers-sim/src/main.rs`**: TCP sim + SQLite migrations
- **`crates/vibe_core/`**: `NetMessage`, `TileKey`, coordinate helpers

## Database Schema

The application uses SQLite with two main tables:

- **regions**: Stores region data with geographic coordinates (latitude, longitude, tile coordinates)
- **prims**: Stores 3D primitive objects with position, rotation, scale, and color

The database is initialized on first run at `data/regions.db` (client `schema.rs` locally; server uses `vibers-sim/migrations/`).

## Troubleshooting

### Build Errors on Steam Deck

If you encounter build errors, reinstall the missing packages:

1. **Missing `libudev.pc`**:
   ```bash
   sudo pacman -S --overwrite '*' systemd-libs
   ```

2. **Missing C compiler/headers (`stdio.h` or `linux/types.h: No such file or directory`)**:
   ```bash
   sudo pacman -S --overwrite '*' glibc base-devel linux-api-headers
   ```

After installing, rebuild:
```bash
cargo clean
cargo build -p vibers-rs --release
```

### Runtime panic in `specialize_material_meshes` / `SkyBoxMaterial`

Bevy 0.16 can panic with `entity_specialization_ticks.get(...).unwrap()` when a `MeshMaterial3d` is first spawned too late in the frame (e.g. skybox from `bevy_atmosphere`’s default **PostUpdate** hook). This project disables that hook (`bevy_atmosphere` without the `detection` feature) and spawns the skybox in **Startup** with the camera (see `free_camera::setup_camera`). If you re-enable automatic skybox detection, expect the same class of crash until you upgrade Bevy or follow upstream [bevy#18980](https://github.com/bevyengine/bevy/issues/18980).

On **Wayland + GLES**, you may see `Re-initializing Gles context`; if rendering misbehaves, try forcing Vulkan: `WGPU_BACKEND=vulkan cargo run -p vibers-rs`.

### Other Issues

- **Database errors**: Ensure the `data/` directory exists and is writable
- **Compilation errors**: Run `cargo clean` and rebuild if you encounter strange build errors
- **Performance issues**: Use `cargo run --release` for better performance (slower compilation but faster runtime)
