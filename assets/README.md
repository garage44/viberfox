# Client runtime assets

`viberfox` loads these from the **workspace root** `assets/` directory (see `AssetPlugin` in `crates/viberfox/src/main.rs`).

- **`models/animated/Fox.glb`** — Bevy’s sample fox mesh and animations (idle / run). Source: [bevyengine/bevy `assets/models/animated`](https://github.com/bevyengine/bevy/tree/main/assets/models/animated).

If the file is missing, restore it with:

```bash
mkdir -p assets/models/animated
curl -fsSL -o assets/models/animated/Fox.glb \
  https://raw.githubusercontent.com/bevyengine/bevy/main/assets/models/animated/Fox.glb
```
