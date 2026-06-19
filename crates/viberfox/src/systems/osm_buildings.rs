//! OSM 3D buildings — increment 1 of the map/sim design (ADR-020/021).
//!
//! Fetches OSM building footprints for the loaded region's tile bbox via the
//! Overpass API (background thread, so the window never blocks), extrudes each
//! footprint to its `height` / `building:levels` tag, and spawns it as a mesh in
//! region-local space. Scoped to the single existing region — no streaming, no
//! floating origin yet; this is the data-quality + extrusion de-risking step.
//!
//! Placement maps each building vertex through the same Web Mercator math the
//! tiles use (fractional tile coords → the region quad), and scales heights by
//! the quad-to-real-meter ratio so buildings sit proportionally on the displayed
//! map regardless of the tile's true ground size.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::components::Region;
use vibe_core::world::{tile_to_lat_lng, tile_to_meters, REGION_SIZE_METERS};

/// One OSM building: an outer-ring footprint in (lat, lng) plus a height in metres.
#[derive(Clone, Debug)]
struct Building {
    ring: Vec<(f64, f64)>,
    height_m: f32,
}

enum Fetch {
    Pending,
    Done(Vec<Building>),
    Failed(String),
}

#[derive(Resource)]
pub struct OsmBuildings {
    result: Arc<Mutex<Fetch>>,
    started: bool,
    spawned: bool,
}

impl Default for OsmBuildings {
    fn default() -> Self {
        Self {
            result: Arc::new(Mutex::new(Fetch::Pending)),
            started: false,
            spawned: false,
        }
    }
}

/// Marker for spawned building meshes.
#[derive(Component)]
pub struct OsmBuildingMesh;

/// The displayed region quad's full edge length (mirrors `rendering::spawn_regions`,
/// which builds `Cuboid::new(REGION_SIZE_METERS/2, .., REGION_SIZE_METERS/2)`).
const GROUND_FULL: f32 = (REGION_SIZE_METERS as f32) / 2.0;

/// Default metres per storey when only `building:levels` is present.
const STOREY_HEIGHT: f32 = 3.0;
/// Fallback height when a building has no height/levels tags at all.
const DEFAULT_HEIGHT: f32 = 6.0;

/// Once a region exists, kick off a background Overpass fetch for its tile bbox.
pub fn start_building_fetch(mut osm: ResMut<OsmBuildings>, regions: Query<&Region>) {
    if osm.started {
        return;
    }
    let Some(region) = regions.iter().next() else {
        return;
    };

    let z = region.tile_z.clamp(0, u32::MAX as i64) as u32;
    // Cover the whole map-tier patch, not just the centre tile, so buildings
    // appear wherever there's ground (placement is relative to the centre tile).
    let r = crate::systems::map_tiles::RADIUS;
    let (north, west) = tile_to_lat_lng(region.tile_x - r, region.tile_y - r, z);
    let (south, east) = tile_to_lat_lng(region.tile_x + r + 1, region.tile_y + r + 1, z);

    let query = format!(
        "[out:json][timeout:25];way[\"building\"]({south},{west},{north},{east});out geom;"
    );

    let result = osm.result.clone();
    osm.started = true;
    tracing::info!(south, west, north, east, "fetching OSM buildings (Overpass)");

    std::thread::spawn(move || {
        let fetched = fetch_buildings(&query);
        let next = match fetched {
            Ok(list) => {
                tracing::info!(count = list.len(), "OSM buildings fetched");
                Fetch::Done(list)
            }
            Err(e) => {
                tracing::error!(error = %e, "OSM buildings fetch failed");
                Fetch::Failed(e)
            }
        };
        if let Ok(mut slot) = result.lock() {
            *slot = next;
        }
    });
}

/// When the fetch completes, extrude and spawn the buildings into region space.
pub fn spawn_buildings(
    mut commands: Commands,
    mut osm: ResMut<OsmBuildings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    regions: Query<&Region>,
) {
    if osm.spawned {
        return;
    }
    let Some(region) = regions.iter().next() else {
        return;
    };

    // Take the fetched buildings out (or bail until ready).
    let buildings = {
        let mut slot = match osm.result.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        match &mut *slot {
            Fetch::Pending => return,
            Fetch::Failed(e) => {
                tracing::error!(error = %e, "OSM buildings unavailable; skipping");
                drop(slot);
                osm.spawned = true; // don't retry
                return;
            }
            Fetch::Done(list) => std::mem::take(list),
        }
    };

    let z = region.tile_z.clamp(0, u32::MAX as i64) as u32;
    let origin = region.sim_origin.unwrap_or(Vec3::ZERO);
    // Quad-to-real-metre ratio: the quad shows a full tile across GROUND_FULL m.
    let real_tile_m = tile_to_meters(z, region.latitude) as f32;
    let height_scale = if real_tile_m > 0.0 {
        GROUND_FULL / real_tile_m
    } else {
        1.0
    };

    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.74, 0.68),
        perceptual_roughness: 0.95,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let mut spawned = 0;
    for b in &buildings {
        // Footprint → region-local XZ.
        let ring: Vec<Vec2> = b
            .ring
            .iter()
            .map(|&(lat, lng)| lat_lng_to_local(lat, lng, region.tile_x, region.tile_y, z))
            .collect();

        let height = b.height_m * height_scale;
        if let Some(mesh) = build_building_mesh(&ring, height) {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(origin + Vec3::Y * 0.05),
                OsmBuildingMesh,
            ));
            spawned += 1;
        }
    }

    osm.spawned = true;
    tracing::info!(spawned, height_scale, "OSM buildings spawned");
}

/// Web Mercator (lat,lng) → region-local metres on the displayed quad.
/// `fx,fy ∈ [0,1]` within the tile; the quad spans `±GROUND_FULL/2`.
/// North is `-Z` (fy grows southward), East is `+X`.
fn lat_lng_to_local(lat: f64, lng: f64, tile_x: i64, tile_y: i64, z: u32) -> Vec2 {
    let n = 2.0_f64.powi(z as i32);
    let x = (lng + 180.0) / 360.0 * n;
    let lat_rad = lat.to_radians();
    let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n;

    let fx = (x - tile_x as f64) as f32;
    let fy = (y - tile_y as f64) as f32;
    Vec2::new((fx - 0.5) * GROUND_FULL, (fy - 0.5) * GROUND_FULL)
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
