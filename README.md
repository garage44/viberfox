# Viberfox

A networked 3D virtual world built with Bevy and Rust. Walk around a map-anchored world, place and edit 3D objects, and share the space with others in real time.

## Prerequisites

- Rust (install via [rustup.rs](https://rustup.rs/))
- On Fedora/Distrobox, run `./setup-fedora.sh` to install system dependencies.

## Running

```bash
cargo run -p viberfox
```

This starts the client in offline mode, loading from a local SQLite database at `data/regions.db`.

To connect to a live sim server:

```bash
cargo run -p viberfox -- --connect <host:port>
```

### Controls

| Input | Action |
|-------|--------|
| W / S | Move forward / backward (camera-relative) |
| A / D | Strafe left / right |
| Left-drag | Orbit camera |
| F | Toggle fly / walk mode |
| Space / Shift | Fly up / down |
| Right-click | Context menu (create, edit, delete prims) |

## Building

```bash
# Release binary
cargo build -p viberfox --release
# Output: target/release/viberfox
```

## Development

### Faster iteration

If the client feels sluggish in debug mode, use the `dev-bevy` profile to compile dependencies at full optimization (slower first build, smoother runtime):

```bash
cargo run -p viberfox --profile dev-bevy
```

For faster incremental **links** during UI/gameplay work:

```bash
cargo run -p viberfox --features fast-dev   # Bevy dynamic linking — local dev only
```

Do not combine `fast-dev` with `--release`.

### Scoped builds

The workspace has three crates. Build only what you changed:

```bash
cargo check -p vibe_core      # shared protocol types
cargo check -p vibe_sim     # server only (no Bevy — fast)
cargo check -p viberfox       # Bevy client
```

### Running the sim server

```bash
cargo run -p vibe_sim
```

Config can be set via `vibe.toml`, environment variables (`VIBE_*`), or CLI flags:

```bash
cargo run -p vibe_sim -- --listen 0.0.0.0:4747 --database-path ./data/regions.db
```

### Assets

The client loads from `assets/` at the workspace root. If `assets/models/animated/Fox.glb` is missing:

```bash
mkdir -p assets/models/animated
curl -fsSL -o assets/models/animated/Fox.glb \
  https://raw.githubusercontent.com/bevyengine/bevy/main/assets/models/animated/Fox.glb
```

## Architecture

Three crates in the workspace:

| Crate | Role |
|-------|------|
| `vibe_core` | Shared protocol types and coordinate math |
| `vibe_sim` | Headless TCP simulation server (Tokio + SQLite) |
| `viberfox` | Bevy 0.16 game client |

The client uses Bevy's ECS. Adding a feature means defining components/resources and registering a system in `main.rs`. Architecture decisions are documented under `meta/adr/`.

## Troubleshooting

**Wayland rendering issues:** try forcing Vulkan:
```bash
WGPU_BACKEND=vulkan cargo run -p viberfox
```

**Missing packages on Steam Deck:**
```bash
sudo pacman -S --overwrite '*' systemd-libs glibc base-devel linux-api-headers
cargo clean && cargo build -p viberfox --release
```

**Build errors:** `cargo clean` and rebuild usually fixes stale incremental state.
