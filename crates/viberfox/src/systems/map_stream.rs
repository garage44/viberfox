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
use bevy::render::view::NoFrustumCulling;
use bevy_image::{Image, ImageSampler};
use big_space::prelude::{BigSpace, Grid, GridCell};

use crate::components::Region;
use crate::resources::OsmTileUrlTemplate;
use crate::systems::free_camera::{FreeCamera, WORLD_CELL_EDGE, WORLD_SWITCH_THRESHOLD};
use crate::systems::tile_loader::load_tile_image;
use vibe_core::world::{tile_to_lat_lng, tile_to_meters};
use vibe_core::TileKey;

/// Fractional Web Mercator tile coordinate (no flooring) of a lat/lng at a zoom.
fn lat_lng_to_tile_frac(lat: f64, lng: f64, zoom: u32) -> (f64, f64) {
    let n = 2.0_f64.powi(zoom as i32);
    let x = (lng + 180.0) / 360.0 * n;
    let lat_rad = lat.to_radians();
    let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
    (x, y)
}

/// Tiles to keep loaded in each direction from the camera (radius 4 → 9×9).
const LOAD_RADIUS: i64 = 4;
/// Lowest (coarsest) zoom we drop to when high up.
const MIN_ZOOM: u32 = 13;
/// Altitude margin a zoom boundary must be crossed by before switching (anti-thrash).
const ZOOM_HYSTERESIS_M: f32 = 60.0;

/// Pick a map zoom from camera altitude: detailed near the ground, coarser high up
/// so a wide area is visible cheaply. Capped at the region's base zoom.
fn zoom_for_altitude(altitude_m: f32, base_zoom: u32) -> u32 {
    let z = if altitude_m < 200.0 {
        base_zoom
    } else if altitude_m < 700.0 {
        base_zoom.saturating_sub(1)
    } else if altitude_m < 2_000.0 {
        base_zoom.saturating_sub(2)
    } else if altitude_m < 6_000.0 {
        base_zoom.saturating_sub(3)
    } else {
        base_zoom.saturating_sub(4)
    };
    z.max(MIN_ZOOM)
}

#[derive(Component)]
pub struct StreamTile {
    key: TileKey,
}

/// Marker: texture applied (or failed) — don't reprocess.
#[derive(Component)]
pub struct StreamTileReady;

#[derive(Resource)]
pub struct MapStream {
    /// Fixed world origin: the *centre* of the region tile at base zoom (lat, lng),
    /// plus the base zoom. Every zoom's tiles are placed relative to this one point,
    /// so changing zoom never shifts the map.
    anchor: Option<(f64, f64, u32)>,
    /// Currently-displayed zoom (driven by camera altitude for LOD).
    current_zoom: u32,
    /// Real ground edge of a tile in metres at the current zoom + anchor latitude.
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
            current_zoom: 0,
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
    // World origin = centre of the region tile (matches the region quad + buildings).
    let (n_lat, w_lng) = tile_to_lat_lng(region.tile_x, region.tile_y, z);
    let (s_lat, e_lng) = tile_to_lat_lng(region.tile_x + 1, region.tile_y + 1, z);
    let clat = (n_lat + s_lat) / 2.0;
    let clng = (w_lng + e_lng) / 2.0;
    let ground = tile_to_meters(z, clat) as f32;
    stream.anchor = Some((clat, clng, z));
    stream.current_zoom = z;
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
            let bytes = match load_tile_image(&key, &template) {
                Ok(b) => Some(b),
                Err(e) => {
                    tracing::warn!(tile = %key.to_path(), error = %e, "map tile fetch failed");
                    None
                }
            };
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
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<(&GridCell, &Transform), With<FreeCamera>>,
    bigspace: Query<Entity, With<BigSpace>>,
) {
    let Some((clat, clng, base_zoom)) = stream.anchor else {
        return;
    };
    let (Some(placeholder), Some(jobs)) = (stream.placeholder.clone(), stream.jobs.clone()) else {
        return;
    };
    let Ok((cell, tf)) = camera.single() else {
        return;
    };
    let Ok(root) = bigspace.single() else {
        return;
    };

    // LOD: choose zoom from camera altitude; rebuild the patch when it changes.
    // Hysteresis: the boundary must be crossed by a margin so hovering can't thrash.
    let altitude = cell.y as f32 * WORLD_CELL_EDGE + tf.translation.y;
    let cur = stream.current_zoom;
    let desired = zoom_for_altitude(altitude, base_zoom);
    let confirmed = if desired < cur {
        zoom_for_altitude(altitude - ZOOM_HYSTERESIS_M, base_zoom) <= desired
    } else if desired > cur {
        zoom_for_altitude(altitude + ZOOM_HYSTERESIS_M, base_zoom) >= desired
    } else {
        false
    };
    if confirmed {
        tracing::info!(from = cur, to = desired, altitude, "map LOD zoom changed");
        for (_, entity) in stream.loaded.drain() {
            commands.entity(entity).despawn();
        }
        let gs = tile_to_meters(desired, clat) as f32;
        stream.current_zoom = desired;
        stream.ground_size = gs;
        stream.quad = Some(meshes.add(Cuboid::new(gs, 0.04, gs)));
    }
    let zoom = stream.current_zoom;
    let Some(quad) = stream.quad.clone() else {
        return;
    };
    let gs = stream.ground_size;

    // Fractional tile coord of the fixed world origin at this zoom. A tile (tx,ty)
    // centre sits at world ((tx+0.5 - fcx)*gs, (ty+0.5 - fcy)*gs), so the origin
    // stays put across zoom changes.
    let (fcx, fcy) = lat_lng_to_tile_frac(clat, clng, zoom);

    // Camera world position → centre tile.
    let world_x = cell.x as f32 * WORLD_CELL_EDGE + tf.translation.x;
    let world_z = cell.z as f32 * WORLD_CELL_EDGE + tf.translation.z;
    let ctx = (world_x as f64 / gs as f64 + fcx - 0.5).round() as i64;
    let cty = (world_z as f64 / gs as f64 + fcy - 0.5).round() as i64;

    // Desired set around the camera.
    let mut desired = HashSet::new();
    for b in -LOAD_RADIUS..=LOAD_RADIUS {
        for a in -LOAD_RADIUS..=LOAD_RADIUS {
            desired.insert(TileKey::new(ctx + a, cty + b, zoom));
        }
    }

    // Spawn newly-needed tiles.
    for key in &desired {
        if stream.loaded.contains_key(key) {
            continue;
        }
        let wx = ((key.x as f64 + 0.5 - fcx) * gs as f64) as f32;
        let wz = ((key.y as f64 + 0.5 - fcy) * gs as f64) as f32;
        // Place under the BigSpace so the tile rebases with the camera (any range).
        let world = Vec3::new(wx, -0.03, wz);
        let (tile_cell, local) =
            Grid::new(WORLD_CELL_EDGE, WORLD_SWITCH_THRESHOLD).translation_to_grid(world.as_dvec3());
        let entity = commands
            .spawn((
                Mesh3d(quad.clone()),
                MeshMaterial3d(placeholder.clone()),
                Transform::from_translation(local),
                tile_cell,
                ChildOf(root),
                // Flat tiles + big_space rebasing confuse frustum culling; opt out.
                NoFrustumCulling,
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
