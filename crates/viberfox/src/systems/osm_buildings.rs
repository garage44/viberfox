//! OSM 3D buildings (ADR-020/022) — a camera-following building patch.
//!
//! Fetches OSM building footprints around the camera from the Overpass API on a
//! background thread, extrudes each footprint to its `height` / `building:levels`
//! tag, and spawns it as a coloured mesh. As the camera roams, the patch is
//! re-fetched and the previous set despawned, so buildings follow the streamed
//! ground (`map_stream`).
//!
//! Placement maps each vertex through the same Web Mercator math as the tiles
//! (relative to the anchor region tile); heights use the interim quad-to-metre
//! scale until the real-metre frame (ADR-019) lands.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use big_space::prelude::{BigSpace, Grid, GridCell};

use crate::components::Region;
use crate::systems::free_camera::{FreeCamera, WORLD_CELL_EDGE, WORLD_SWITCH_THRESHOLD};
use vibe_core::world::{tile_to_lat_lng, tile_to_meters};

/// One OSM building: an outer-ring footprint in (lat, lng) plus a height in metres.
#[derive(Clone, Debug)]
struct Building {
    ring: Vec<(f64, f64)>,
    height_m: f32,
}

/// Camera-following building patch: re-fetched from Overpass when the camera
/// drifts, with the old set despawned (ADR-020/022). Anchored to the region tile
/// so buildings stay aligned with the streamed ground.
#[derive(Resource, Default)]
pub struct OsmBuildings {
    /// Region anchor: tile_x, tile_y, zoom, latitude.
    anchor: Option<(i64, i64, u32, f64)>,
    /// Centre tile of the last fetch (re-fetch when the camera moves away).
    last_center: Option<(i64, i64)>,
    in_flight: bool,
    /// Latest completed fetch result, consumed on the main thread.
    fetch: Arc<Mutex<Option<Result<Vec<Building>, String>>>>,
    /// Currently-spawned building meshes (despawned on re-fetch).
    entities: Vec<Entity>,
}

/// Marker for spawned building meshes.
#[derive(Component)]
pub struct OsmBuildingMesh;

/// Default metres per storey when only `building:levels` is present.
const STOREY_HEIGHT: f32 = 3.0;
/// Fallback height when a building has no height/levels tags at all.
const DEFAULT_HEIGHT: f32 = 6.0;

/// Tiles out from the camera covered by the building patch.
const BUILDING_PATCH_RADIUS: i64 = 3;
/// Re-fetch the patch once the camera has moved this many tiles from the last centre.
const REFETCH_TILES: i64 = 2;

/// Muted, realistic building tones (tans, greys, beige, muted brick, off-white).
const BUILDING_COLORS: [[f32; 3]; 8] = [
    [0.80, 0.76, 0.70],
    [0.72, 0.70, 0.66],
    [0.78, 0.72, 0.64],
    [0.68, 0.66, 0.64],
    [0.76, 0.63, 0.56],
    [0.83, 0.81, 0.77],
    [0.70, 0.65, 0.58],
    [0.74, 0.71, 0.69],
];

/// Deterministic palette index from a building's location, so the same building
/// always gets the same colour and neighbours differ.
fn building_color_index(b: &Building) -> usize {
    let (lat, lng) = b.ring[0];
    let h = ((lat * 1.0e5) as i64)
        .wrapping_mul(73_856_093)
        ^ ((lng * 1.0e5) as i64).wrapping_mul(19_349_663);
    h.unsigned_abs() as usize
}

/// Re-fetch and respawn the building patch as the camera roams (ADR-020/022).
pub fn update_buildings(
    mut commands: Commands,
    mut osm: ResMut<OsmBuildings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    regions: Query<&Region>,
    camera: Query<(&GridCell, &Transform), With<FreeCamera>>,
    bigspace: Query<Entity, With<BigSpace>>,
) {
    // Establish the region anchor once.
    if osm.anchor.is_none() {
        let Some(region) = regions.iter().next() else {
            return;
        };
        let z = region.tile_z.clamp(0, u32::MAX as i64) as u32;
        osm.anchor = Some((region.tile_x, region.tile_y, z, region.latitude));
    }
    let (ax, ay, z, lat) = osm.anchor.unwrap();
    let tile_m = tile_to_meters(z, lat) as f32;

    // Camera's current tile (world position → tile offset from the anchor).
    let Ok((cell, tf)) = camera.single() else {
        return;
    };
    let world_x = cell.x as f32 * WORLD_CELL_EDGE + tf.translation.x;
    let world_z = cell.z as f32 * WORLD_CELL_EDGE + tf.translation.z;
    let center = (
        ax + (world_x / tile_m).round() as i64,
        ay + (world_z / tile_m).round() as i64,
    );

    // Kick off a re-fetch when the camera has drifted far enough.
    let need = match osm.last_center {
        None => true,
        Some((lx, ly)) => (center.0 - lx).abs() + (center.1 - ly).abs() >= REFETCH_TILES,
    };
    if need && !osm.in_flight {
        let r = BUILDING_PATCH_RADIUS;
        let (north, west) = tile_to_lat_lng(center.0 - r, center.1 - r, z);
        let (south, east) = tile_to_lat_lng(center.0 + r + 1, center.1 + r + 1, z);
        let query = format!(
            "[out:json][timeout:25];way[\"building\"]({south},{west},{north},{east});out geom;"
        );
        osm.in_flight = true;
        osm.last_center = Some(center);
        let slot = osm.fetch.clone();
        std::thread::spawn(move || {
            let result = fetch_buildings(&query);
            if let Ok(mut g) = slot.lock() {
                *g = Some(result);
            }
        });
    }

    // Apply a completed fetch: despawn the old patch, spawn the new one.
    let ready = osm.fetch.lock().ok().and_then(|mut g| g.take());
    let Some(result) = ready else {
        return;
    };
    osm.in_flight = false;
    let buildings = match result {
        Ok(list) => list,
        Err(e) => {
            tracing::error!(error = %e, "OSM buildings fetch failed");
            return;
        }
    };

    for entity in osm.entities.drain(..) {
        commands.entity(entity).despawn();
    }

    // Per-location palette so the city reads as distinct buildings.
    let palette: Vec<Handle<StandardMaterial>> = BUILDING_COLORS
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(c[0], c[1], c[2]),
                perceptual_roughness: 0.95,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        })
        .collect();

    let Ok(root) = bigspace.single() else {
        return;
    };
    for b in &buildings {
        // World footprint, then recentre on the first vertex so the mesh is local
        // and the entity can carry a BigSpace GridCell (rebases at any range).
        let ring_world: Vec<Vec2> = b
            .ring
            .iter()
            .map(|&(la, lo)| lat_lng_to_local(la, lo, ax, ay, z, tile_m))
            .collect();
        let r0 = ring_world[0];
        let ring: Vec<Vec2> = ring_world.iter().map(|p| *p - r0).collect();
        let height = b.height_m;
        if let Some(mesh) = build_building_mesh(&ring, height) {
            let mat = palette[building_color_index(b) % palette.len()].clone();
            let world = Vec3::new(r0.x, 0.05, r0.y);
            let (cell, local) = Grid::new(WORLD_CELL_EDGE, WORLD_SWITCH_THRESHOLD)
                .translation_to_grid(world.as_dvec3());
            let entity = commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(mat),
                    Transform::from_translation(local),
                    cell,
                    ChildOf(root),
                    OsmBuildingMesh,
                ))
                .id();
            osm.entities.push(entity);
        }
    }
    tracing::info!(count = osm.entities.len(), "buildings updated");
}

/// Web Mercator (lat,lng) → world metres relative to the anchor tile centre.
/// `tile_m` is the real ground edge of a tile; East is `+X`, South is `+Z`.
fn lat_lng_to_local(lat: f64, lng: f64, tile_x: i64, tile_y: i64, z: u32, tile_m: f32) -> Vec2 {
    let n = 2.0_f64.powi(z as i32);
    let x = (lng + 180.0) / 360.0 * n;
    let lat_rad = lat.to_radians();
    let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;

    let fx = (x - tile_x as f64) as f32;
    let fy = (y - tile_y as f64) as f32;
    Vec2::new((fx - 0.5) * tile_m, (fy - 0.5) * tile_m)
}

/// Extrude a footprint ring (region-local XZ) to a solid: triangulated top cap +
/// vertical walls. Returns `None` for degenerate footprints.
fn build_building_mesh(ring_in: &[Vec2], height: f32) -> Option<Mesh> {
    // Drop a duplicated closing vertex if present.
    let mut ring = ring_in.to_vec();
    if ring.len() >= 2 && ring.first() == ring.last() {
        ring.pop();
    }
    let n = ring.len();
    if n < 3 || height <= 0.0 {
        return None;
    }

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // --- Top cap (triangulated via earcut over XZ) ---
    let flat: Vec<f64> = ring.iter().flat_map(|p| [p.x as f64, p.y as f64]).collect();
    let cap = earcutr::earcut(&flat, &[], 2).ok()?;
    let top_base = 0u32;
    for p in &ring {
        positions.push([p.x, height, p.y]);
        normals.push([0.0, 1.0, 0.0]);
    }
    for tri in cap {
        indices.push(top_base + tri as u32);
    }

    // Footprint centroid (XZ) — used to orient wall normals outward regardless
    // of the OSM ring's winding (which isn't guaranteed).
    let centroid = ring.iter().copied().sum::<Vec2>() / n as f32;

    // --- Walls (one quad per edge) ---
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let edge = b - a;
        // Perpendicular to the edge in XZ, flipped to point away from the centroid.
        let mut perp = Vec2::new(edge.y, -edge.x).normalize_or_zero();
        let mid = (a + b) * 0.5;
        if perp.dot(mid - centroid) < 0.0 {
            perp = -perp;
        }
        let nrm = Vec3::new(perp.x, 0.0, perp.y);
        let base = positions.len() as u32;
        // a_bottom, b_bottom, b_top, a_top
        positions.push([a.x, 0.0, a.y]);
        positions.push([b.x, 0.0, b.y]);
        positions.push([b.x, height, b.y]);
        positions.push([a.x, height, a.y]);
        for _ in 0..4 {
            normals.push([nrm.x, nrm.y, nrm.z]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

// --- Overpass fetch + parse (runs on the background thread) ---

fn fetch_buildings(query: &str) -> Result<Vec<Building>, String> {
    let resp = ureq::post("https://overpass-api.de/api/interpreter")
        .send_form(&[("data", query)])
        .map_err(|e| e.to_string())?;
    let text = resp.into_string().map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    let elements = json
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "no elements in Overpass response".to_string())?;

    let mut out = Vec::new();
    for el in elements {
        if el.get("type").and_then(|t| t.as_str()) != Some("way") {
            continue;
        }
        let Some(geom) = el.get("geometry").and_then(|g| g.as_array()) else {
            continue;
        };
        let ring: Vec<(f64, f64)> = geom
            .iter()
            .filter_map(|pt| {
                Some((pt.get("lat")?.as_f64()?, pt.get("lon")?.as_f64()?))
            })
            .collect();
        if ring.len() < 3 {
            continue;
        }
        let height_m = parse_height(el.get("tags"));
        out.push(Building { ring, height_m });
    }
    Ok(out)
}

/// Height from `height` (metres, optional " m" suffix) → `building:levels` × 3 m → default.
fn parse_height(tags: Option<&serde_json::Value>) -> f32 {
    let Some(tags) = tags else {
        return DEFAULT_HEIGHT;
    };
    if let Some(h) = tags.get("height").and_then(num_from_value) {
        return h;
    }
    if let Some(levels) = tags.get("building:levels").and_then(num_from_value) {
        return levels * STOREY_HEIGHT;
    }
    DEFAULT_HEIGHT
}

/// Parse a leading number from a JSON value that may be a number or a string like "12 m".
fn num_from_value(v: &serde_json::Value) -> Option<f32> {
    if let Some(n) = v.as_f64() {
        return Some(n as f32);
    }
    let s = v.as_str()?;
    let num: String = s
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num.parse::<f32>().ok()
}
