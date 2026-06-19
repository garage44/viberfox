//! Map-tier ground patch (ADR-020): a grid of OSM tiles around the region so you
//! see the surrounding city, not just the single sim tile. Static for now (no
//! quadtree/LOD yet) and near the origin, so no big_space `GridCell` is needed.
//!
//! Tiles are fetched on ONE background thread (progressively popping in) to keep
//! the window responsive — the shared `tile_loader` path is blocking and would
//! stall on a whole grid. This is the seed of the streamed map tier.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_image::{Image, ImageSampler};

use crate::components::Region;
use crate::resources::OsmTileUrlTemplate;
use crate::systems::tile_loader::load_tile_image;
use vibe_core::world::REGION_SIZE_METERS;
use vibe_core::TileKey;

/// How many tiles out from the centre region (radius 2 → 5×5 grid). Shared with
/// the buildings loader so buildings cover the same area as the ground.
pub const RADIUS: i64 = 2;
/// Ground quad edge — mirrors the region quad in `rendering::spawn_regions`
/// (`Cuboid::new(REGION_SIZE_METERS/2, .., REGION_SIZE_METERS/2)`), so tiles abut.
const GROUND_FULL: f32 = (REGION_SIZE_METERS as f32) / 2.0;

/// A map-tier ground tile (distinct from a sim `Region`).
#[derive(Component)]
pub struct MapTile {
    key: TileKey,
}

/// Marker: this tile's texture has been applied (or it failed) — don't reprocess.
#[derive(Component)]
pub struct MapTileReady;

/// Background fetch results: `None` = failed, `Some(bytes)` = ready, absent = pending.
#[derive(Resource, Default)]
pub struct MapTiles {
    results: Arc<Mutex<HashMap<TileKey, Option<Vec<u8>>>>>,
    started: bool,
}

/// Spawn the surrounding ground tiles once a region exists, and kick off a single
/// background thread that fetches their textures in sequence.
pub fn spawn_map_grid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut map: ResMut<MapTiles>,
    regions: Query<&Region>,
    tile_url: Res<OsmTileUrlTemplate>,
) {
    if map.started {
        return;
    }
    let Some(region) = regions.iter().next() else {
        return;
    };
    map.started = true;

    let z = region.tile_z.clamp(0, u32::MAX as i64) as u32;
    let (cx, cy) = (region.tile_x, region.tile_y);
    let quad = meshes.add(Cuboid::new(GROUND_FULL, 0.04, GROUND_FULL));

    let mut keys = Vec::new();
    for dj in -RADIUS..=RADIUS {
        for di in -RADIUS..=RADIUS {
            if di == 0 && dj == 0 {
                continue; // the sim region already covers the centre tile
            }
            let key = TileKey::new(cx + di, cy + dj, z);
            keys.push(key.clone());
            // Placeholder grey until the texture arrives; +X = east (di), +Z = south (dj).
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.6, 0.6, 0.6),
                ..default()
            });
            commands.spawn((
                Mesh3d(quad.clone()),
                MeshMaterial3d(mat),
                Transform::from_xyz(di as f32 * GROUND_FULL, -0.03, dj as f32 * GROUND_FULL),
                MapTile { key },
            ));
        }
    }

    let template = tile_url.0.lock().map(|g| g.clone()).unwrap_or_default();
    let results = map.results.clone();
    tracing::info!(tiles = keys.len(), "fetching map-tier tiles");
    std::thread::spawn(move || {
        for key in keys {
            let bytes = load_tile_image(&key, &template).ok();
            if let Ok(mut g) = results.lock() {
                g.insert(key, bytes);
            }
        }
        tracing::info!("map-tier tile fetch complete");
    });
}

/// Apply fetched textures to map tiles as they arrive (tiles pop in progressively).
pub fn apply_map_tiles(
    mut commands: Commands,
    map: Res<MapTiles>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tiles: Query<(Entity, &MapTile), Without<MapTileReady>>,
) {
    let Ok(results) = map.results.lock() else {
        return;
    };
    for (entity, tile) in tiles.iter() {
        match results.get(&tile.key) {
            None => continue, // still pending
            Some(None) => {
                commands.entity(entity).insert(MapTileReady); // failed; stop retrying
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
                commands.entity(entity).insert(MapTileReady);
            }
        }
    }
}
