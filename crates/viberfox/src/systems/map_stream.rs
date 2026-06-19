//! Streaming map tier (ADR-020): a camera-following grid of OSM ground tiles that
//! load as you roam and unload behind you. Supersedes the static patch.
//!
//! Stage 1: tiles are root entities in the world frame. This is correct while the
//! camera stays within the home big_space cell (~14 km of ground — ample for a
//! city). Stage 2 will place them under the `BigSpace` with `GridCell`s for
//! unlimited range, and stream buildings per tile.
//!
//! Textures are fetched on a background worker (one Overpass/tile HTTP call per
//! tile, via an mpsc queue) so roaming never blocks the frame.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_image::{Image, ImageSampler};
use big_space::prelude::{BigSpace, Grid, GridCell};

use crate::components::Region;
use crate::resources::OsmTileUrlTemplate;
use crate::systems::free_camera::{FreeCamera, WORLD_CELL_EDGE, WORLD_SWITCH_THRESHOLD};
use crate::systems::tile_loader::load_tile_image;
use vibe_core::world::tile_to_meters;
use vibe_core::TileKey;

/// Tiles to keep loaded in each direction from the camera (radius 4 → 9×9).
const LOAD_RADIUS: i64 = 4;

#[derive(Component)]
pub struct StreamTile {
    key: TileKey,
}

/// Marker: texture applied (or failed) — don't reprocess.
#[derive(Component)]
pub struct StreamTileReady;

#[derive(Resource)]
pub struct MapStream {
    /// Anchor tile (the region's tile) + zoom; world origin sits at its centre.
    anchor: Option<(i64, i64, u32)>,
    /// Real ground edge of a tile in metres at the anchor latitude.
    ground_size: f32,
    loaded: HashMap<TileKey, Entity>,
    /// Tiles ever queued for fetch (kept as a cache so re-entry doesn't refetch).
    requested: HashSet<TileKey>,
    results: Arc<Mutex<HashMap<TileKey, Option<Vec<u8>>>>>,
    jobs: Option<Sender<TileKey>>,
    quad: Option<Handle<Mesh>>,
    placeholder: Option<Handle<StandardMaterial>>,
}

impl Default for MapStream {
    fn default() -> Self {
        Self {
            anchor: None,
            ground_size: 0.0,
            loaded: HashMap::new(),
            requested: HashSet::new(),
            results: Arc::new(Mutex::new(HashMap::new())),
            jobs: None,
            quad: None,
            placeholder: None,
        }
    }
}

/// Once a region exists, set the anchor and spawn the background fetch worker.
pub fn init_map_stream(
    mut stream: ResMut<MapStream>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    regions: Query<&Region>,
    tile_url: Res<OsmTileUrlTemplate>,
) {
    if stream.anchor.is_some() {
        return;
    }
    let Some(region) = regions.iter().next() else {
        return;
    };
    let z = region.tile_z.clamp(0, u32::MAX as i64) as u32;
    let ground = tile_to_meters(z, region.latitude) as f32;
    stream.anchor = Some((region.tile_x, region.tile_y, z));
    stream.ground_size = ground;
    stream.quad = Some(meshes.add(Cuboid::new(ground, 0.04, ground)));
    stream.placeholder = Some(materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.6, 0.6),
        ..default()
    }));

    let (tx, rx) = std::sync::mpsc::channel::<TileKey>();
    stream.jobs = Some(tx);
    let template = tile_url.0.lock().map(|g| g.clone()).unwrap_or_default();
    let results = stream.results.clone();
    std::thread::spawn(move || {
        while let Ok(key) = rx.recv() {
            let bytes = load_tile_image(&key, &template).ok();
            if let Ok(mut g) = results.lock() {
                g.insert(key, bytes);
            }
        }
    });
    tracing::info!("map streaming initialized");
}

/// Spawn ground tiles within `LOAD_RADIUS` of the camera; despawn the rest.
pub fn update_map_stream(
    mut commands: Commands,
    mut stream: ResMut<MapStream>,
    camera: Query<(&GridCell, &Transform), With<FreeCamera>>,
    bigspace: Query<Entity, With<BigSpace>>,
) {
    let Some((ax, ay, z)) = stream.anchor else {
        return;
    };
    let (Some(quad), Some(placeholder), Some(jobs)) =
        (stream.quad.clone(), stream.placeholder.clone(), stream.jobs.clone())
    else {
        return;
    };
    let Ok((cell, tf)) = camera.single() else {
        return;
    };
    let Ok(root) = bigspace.single() else {
        return;
    };
    let gs = stream.ground_size;

    // Camera world position (cell index × cell edge + within-cell offset).
    let world_x = cell.x as f32 * WORLD_CELL_EDGE + tf.translation.x;
    let world_z = cell.z as f32 * WORLD_CELL_EDGE + tf.translation.z;
    let cdi = (world_x / gs).round() as i64;
    let cdj = (world_z / gs).round() as i64;
    let (ctx, cty) = (ax + cdi, ay + cdj);

    // Desired set around the camera (skip the anchor tile — the region covers it).
    let mut desired = HashSet::new();
    for b in -LOAD_RADIUS..=LOAD_RADIUS {
        for a in -LOAD_RADIUS..=LOAD_RADIUS {
            let key = TileKey::new(ctx + a, cty + b, z);
            if key.x == ax && key.y == ay {
                continue;
            }
            desired.insert(key);
        }
    }

    // Spawn newly-needed tiles.
    for key in &desired {
        if stream.loaded.contains_key(key) {
            continue;
        }
        let di = (key.x - ax) as f32;
        let dj = (key.y - ay) as f32;
        // Place under the BigSpace so the tile rebases with the camera (any range).
        let world = Vec3::new(di * gs, -0.03, dj * gs);
        let (tile_cell, local) =
            Grid::new(WORLD_CELL_EDGE, WORLD_SWITCH_THRESHOLD).translation_to_grid(world.as_dvec3());
        let entity = commands
            .spawn((
                Mesh3d(quad.clone()),
                MeshMaterial3d(placeholder.clone()),
                Transform::from_translation(local),
                tile_cell,
                ChildOf(root),
                StreamTile { key: key.clone() },
            ))
            .id();
        stream.loaded.insert(key.clone(), entity);
        if stream.requested.insert(key.clone()) {
            let _ = jobs.send(key.clone());
        }
    }

    // Despawn tiles that drifted out of range (texture bytes stay cached).
    let stale: Vec<TileKey> = stream
        .loaded
        .keys()
        .filter(|k| !desired.contains(*k))
        .cloned()
        .collect();
    for key in stale {
        if let Some(entity) = stream.loaded.remove(&key) {
            commands.entity(entity).despawn();
        }
    }
}

/// Apply fetched textures to streamed tiles as they arrive.
pub fn apply_stream_textures(
    mut commands: Commands,
    stream: Res<MapStream>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tiles: Query<(Entity, &StreamTile), Without<StreamTileReady>>,
) {
    let Ok(results) = stream.results.lock() else {
        return;
    };
    for (entity, tile) in tiles.iter() {
        match results.get(&tile.key) {
            None => continue,
            Some(None) => {
                commands.entity(entity).insert(StreamTileReady);
            }
            Some(Some(bytes)) => {
                if let Ok(img) = image::load_from_memory(bytes) {
                    let rgba = img.to_rgba8();
                    let size = Extent3d {
                        width: rgba.width(),
                        height: rgba.height(),
                        depth_or_array_layers: 1,
                    };
                    let mut bevy_image = Image::new(
                        size,
                        TextureDimension::D2,
                        rgba.into_raw(),
                        TextureFormat::Rgba8UnormSrgb,
                        RenderAssetUsages::default(),
                    );
                    bevy_image.sampler = ImageSampler::linear();
                    let handle = images.add(bevy_image);
                    let mat = materials.add(StandardMaterial {
                        base_color_texture: Some(handle),
                        ..default()
                    });
                    commands.entity(entity).insert(MeshMaterial3d(mat));
                }
                commands.entity(entity).insert(StreamTileReady);
            }
        }
    }
}
