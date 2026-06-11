use crate::components::{NeedsMeshRebuild, NeedsTextureRefresh, Prim, PrimShape, Region};
use crate::resources::PrimTextureCache;
use crate::systems::tile_loader::{RegionTile, TileKey};
use bevy::math::primitives::{Cuboid, Cylinder, Sphere, Torus};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use vibe_core::world::REGION_SIZE_METERS;

#[derive(Component)]
pub struct RegionMesh;

#[derive(Component)]
pub struct PrimMesh;

/// Spawn region meshes with tile loading setup
pub fn spawn_regions(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    region_query: Query<(Entity, &Region), (Without<RegionMesh>, Without<Prim>)>,
    all_regions: Query<&Region>,
) {
    // Calculate grid size based on total regions
    let total_regions = all_regions.iter().count();
    let query_count = region_query.iter().count();

    if total_regions == 0 {
        return; // No regions to spawn
    }

    if query_count == 0 && total_regions > 0 {
        // All region entities already have meshes (or are filtered out) — normal steady state.
        return;
    }

    let grid_size = (total_regions as f32).sqrt().ceil();

    // Collect all region IDs to calculate index
    let mut region_ids: Vec<i64> = all_regions.iter().map(|r| r.id).collect();
    region_ids.sort();

    let mut spawned_count = 0;
    for (entity, region) in region_query.iter() {
        spawned_count += 1;
        let position = if let Some(p) = region.sim_origin {
            p
        } else if total_regions == 1 {
            Vec3::new(0.0, 0.0, 0.0)
        } else {
            let index = region_ids
                .iter()
                .position(|&id| id == region.id)
                .unwrap_or(0);
            let row = (index as f32 / grid_size).floor() as i32;
            let col = index % grid_size as usize;
            let spacing = 300.0;
            Vec3::new(
                (col as f32 - grid_size / 2.0) * spacing,
                0.0,
                (row as f32 - grid_size / 2.0) * spacing,
            )
        };

        tracing::info!(name = %region.name, position = ?position, "spawning region mesh");

        // Create a simple flat box as the region (easier than plane rotation)
        // Box with very small height to act as a flat plane
        let region_size = REGION_SIZE_METERS as f32;
        let region_mesh = meshes.add(Cuboid::new(
            region_size / 2.0,
            0.05, // Very thin - acts like a plane
            region_size / 2.0,
        ));

        // Create simple untextured material
        let default_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7), // Light gray
            ..default()
        });

        // Create tile key for this region
        let tile_key = TileKey::new(
            region.tile_x,
            region.tile_y,
            region.tile_z.clamp(0, u32::MAX as i64) as u32,
        );

        // Spawn region as a flat box at y=0
        let transform = Transform::from_translation(position);

        commands.entity(entity).insert((
            Mesh3d(region_mesh),
            MeshMaterial3d(default_material),
            transform,
            Visibility::Visible,
            RegionMesh,
            RegionTile {
                tile_key,
                lod_level: 1, // Start with medium quality
            },
        ));
    }

    if spawned_count > 0 {
        tracing::info!(count = spawned_count, "spawned region meshes");
    }
}

/// Update region materials when tile textures are loaded
pub fn update_region_materials(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut region_query: Query<
        (
            &mut MeshMaterial3d<StandardMaterial>,
            &crate::systems::tile_loader::RegionTileTexture,
        ),
        (
            With<RegionMesh>,
            Changed<crate::systems::tile_loader::RegionTileTexture>,
        ),
    >,
    images: Res<Assets<Image>>,
) {
    for (mut material, tile_texture) in region_query.iter_mut() {
        if images.get(&tile_texture.handle).is_some() {
            // Create new material with tile texture
            let new_material = materials.add(StandardMaterial {
                base_color_texture: Some(tile_texture.handle.clone()),
                ..default()
            });
            *material = MeshMaterial3d(new_material);
        }
    }
}

pub fn spawn_prims(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    prim_query: Query<(Entity, &Prim, &Transform), (Without<PrimMesh>, Without<RegionMesh>)>,
    texture_cache: Res<PrimTextureCache>,
) {
    for (entity, prim, transform) in prim_query.iter() {
        let mesh_handle = prim_mesh_handle(prim, &mut meshes);

        let base_color_texture = prim
            .texture_id
            .as_ref()
            .and_then(|id| texture_cache.handles.get(id))
            .cloned();

        let material_handle = materials.add(StandardMaterial {
            base_color: prim.color,
            base_color_texture,
            ..default()
        });

        commands.entity(entity).insert((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material_handle),
            *transform,
            PrimMesh,
        ));
    }
}

/// Rebuild meshes for prims whose geometry (shape/path-cut/hollow) changed.
pub fn rebuild_prim_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<(Entity, &Prim), With<NeedsMeshRebuild>>,
) {
    for (entity, prim) in query.iter() {
        let mesh_handle = prim_mesh_handle(prim, &mut meshes);
        commands
            .entity(entity)
            .insert(Mesh3d(mesh_handle))
            .remove::<NeedsMeshRebuild>();
    }
}

/// Swap the material texture for prims that were marked after a `TextureData` message arrived.
pub fn refresh_prim_textures(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &Prim, &MeshMaterial3d<StandardMaterial>), With<NeedsTextureRefresh>>,
    cache: Res<PrimTextureCache>,
) {
    for (entity, prim, mat_handle) in query.iter() {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.base_color_texture = prim
                .texture_id
                .as_ref()
                .and_then(|id| cache.handles.get(id))
                .cloned();
        }
        commands.entity(entity).remove::<NeedsTextureRefresh>();
    }
}

// ---------------------------------------------------------------------------
// Mesh builders
// ---------------------------------------------------------------------------

fn prim_mesh_handle(prim: &Prim, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    let begin = prim.path_cut_begin.clamp(0.0, 1.0);
    let end = prim.path_cut_end.clamp(0.0, 1.0);
    let hollow = prim.hollow.clamp(0.0, 0.95);

    let needs_custom = begin > 0.001 || end < 0.999 || hollow > 0.001;

    if !needs_custom {
        return match prim.shape {
            PrimShape::Box => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            PrimShape::Sphere => meshes.add(Sphere::new(0.5)),
            PrimShape::Cylinder => meshes.add(Cylinder::new(0.5, 1.0)),
            PrimShape::Cone => meshes.add(Cylinder::new(0.0, 1.0)),
            PrimShape::Torus => meshes.add(Torus::default()),
        };
    }

    match prim.shape {
        PrimShape::Cylinder => {
            meshes.add(build_cylinder_like_mesh(0.5, 0.5, begin, end, hollow))
        }
        PrimShape::Cone => {
            meshes.add(build_cylinder_like_mesh(0.0, 0.5, begin, end, hollow))
        }
        PrimShape::Box => meshes.add(build_box_mesh(begin, end, hollow)),
        PrimShape::Sphere => meshes.add(build_sphere_mesh(begin, end, hollow)),
        // Torus: path-cut/hollow stored but not yet reflected in mesh.
        PrimShape::Torus => meshes.add(Torus::default()),
    }
}

/// Cylinder or cone mesh with optional angular path cut and hollow.
///
/// `top_r` = top-face radius (0 for cone, 0.5 for cylinder).
/// `bot_r` = bottom-face radius (always 0.5).
/// Angles sweep counterclockwise around the Y axis.
fn build_cylinder_like_mesh(
    top_r: f32,
    bot_r: f32,
    path_cut_begin: f32,
    path_cut_end: f32,
    hollow: f32,
) -> Mesh {
    use std::f32::consts::TAU;

    let segments: usize = 32;
    let inner_top_r = top_r * hollow;
    let inner_bot_r = bot_r * hollow;
    let is_hollow = hollow > 0.001;

    let begin_a = path_cut_begin * TAU;
    let end_a = path_cut_end * TAU;
    let sweep = (end_a - begin_a).max(0.0);
    let is_full = sweep >= TAU - 0.001;

    let arc_steps = if is_full {
        segments
    } else {
        ((sweep / TAU * segments as f32).round() as usize).max(1)
    };

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // add_quad: two triangles (a,b,c) and (a,c,d).
    let add_quad = |indices: &mut Vec<u32>, a: u32, b: u32, c: u32, d: u32| {
        indices.extend_from_slice(&[a, b, c, a, c, d]);
    };

    // ── Outer wall ────────────────────────────────────────────────────────────
    // Vertices: pairs (top, bot) for each arc step.
    let outer_wall_base = positions.len() as u32;
    for i in 0..=arc_steps {
        let t = i as f32 / arc_steps as f32;
        let a = begin_a + t * sweep;
        let (cos_a, sin_a) = (a.cos(), a.sin());
        let r_top = top_r;
        let r_bot = bot_r;
        // top
        positions.push([r_top * cos_a, 0.5, r_top * sin_a]);
        normals.push([cos_a, 0.0, sin_a]);
        uvs.push([t, 0.0]);
        // bottom
        positions.push([r_bot * cos_a, -0.5, r_bot * sin_a]);
        normals.push([cos_a, 0.0, sin_a]);
        uvs.push([t, 1.0]);
    }
    for i in 0..arc_steps {
        let b = outer_wall_base + (i * 2) as u32;
        // verified: top_i, top_{i+1}, bot_{i+1}, bot_i → outward normal
        add_quad(&mut indices, b, b + 2, b + 3, b + 1);
    }

    // ── Inner wall (hollow only) ───────────────────────────────────────────────
    if is_hollow {
        let inner_wall_base = positions.len() as u32;
        for i in 0..=arc_steps {
            let t = i as f32 / arc_steps as f32;
            let a = begin_a + t * sweep;
            let (cos_a, sin_a) = (a.cos(), a.sin());
            // top (inner normal points inward)
            positions.push([inner_top_r * cos_a, 0.5, inner_top_r * sin_a]);
            normals.push([-cos_a, 0.0, -sin_a]);
            uvs.push([t, 0.0]);
            // bottom
            positions.push([inner_bot_r * cos_a, -0.5, inner_bot_r * sin_a]);
            normals.push([-cos_a, 0.0, -sin_a]);
            uvs.push([t, 1.0]);
        }
        for i in 0..arc_steps {
            let b = inner_wall_base + (i * 2) as u32;
            // reversed winding for inward normal
            add_quad(&mut indices, b, b + 1, b + 3, b + 2);
        }
    }

    // ── Top cap ───────────────────────────────────────────────────────────────
    if top_r > 0.001 {
        if !is_hollow {
            // Fan from center
            let center = positions.len() as u32;
            positions.push([0.0, 0.5, 0.0]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.5, 0.5]);
            let ring_start = positions.len() as u32;
            for i in 0..=arc_steps {
                let t = i as f32 / arc_steps as f32;
                let a = begin_a + t * sweep;
                let (cos_a, sin_a) = (a.cos(), a.sin());
                positions.push([top_r * cos_a, 0.5, top_r * sin_a]);
                normals.push([0.0, 1.0, 0.0]);
                uvs.push([0.5 + 0.5 * cos_a, 0.5 + 0.5 * sin_a]);
            }
            for i in 0..arc_steps {
                let v1 = ring_start + i as u32;
                let v2 = ring_start + i as u32 + 1;
                // reversed winding → upward normal (verified above)
                indices.extend_from_slice(&[center, v2, v1]);
            }
        } else {
            // Annulus
            let ann_base = positions.len() as u32;
            for i in 0..=arc_steps {
                let t = i as f32 / arc_steps as f32;
                let a = begin_a + t * sweep;
                let (cos_a, sin_a) = (a.cos(), a.sin());
                positions.push([top_r * cos_a, 0.5, top_r * sin_a]);
                normals.push([0.0, 1.0, 0.0]);
                uvs.push([0.5 + 0.5 * cos_a, 0.5 + 0.5 * sin_a]);
                positions.push([inner_top_r * cos_a, 0.5, inner_top_r * sin_a]);
                normals.push([0.0, 1.0, 0.0]);
                uvs.push([0.5 + inner_top_r * cos_a, 0.5 + inner_top_r * sin_a]);
            }
            for i in 0..arc_steps {
                let b = ann_base + (i * 2) as u32;
                // outer_i, inner_i, inner_{i+1}, outer_{i+1} → upward normal (verified)
                add_quad(&mut indices, b, b + 1, b + 3, b + 2);
            }
        }
    }

    // ── Bottom cap ────────────────────────────────────────────────────────────
    {
        if !is_hollow {
            let center = positions.len() as u32;
            positions.push([0.0, -0.5, 0.0]);
            normals.push([0.0, -1.0, 0.0]);
            uvs.push([0.5, 0.5]);
            let ring_start = positions.len() as u32;
            for i in 0..=arc_steps {
                let t = i as f32 / arc_steps as f32;
                let a = begin_a + t * sweep;
                let (cos_a, sin_a) = (a.cos(), a.sin());
                positions.push([bot_r * cos_a, -0.5, bot_r * sin_a]);
                normals.push([0.0, -1.0, 0.0]);
                uvs.push([0.5 + 0.5 * cos_a, 0.5 + 0.5 * sin_a]);
            }
            for i in 0..arc_steps {
                let v1 = ring_start + i as u32;
                let v2 = ring_start + i as u32 + 1;
                // forward winding → downward normal (verified)
                indices.extend_from_slice(&[center, v1, v2]);
            }
        } else {
            let ann_base = positions.len() as u32;
            for i in 0..=arc_steps {
                let t = i as f32 / arc_steps as f32;
                let a = begin_a + t * sweep;
                let (cos_a, sin_a) = (a.cos(), a.sin());
                positions.push([bot_r * cos_a, -0.5, bot_r * sin_a]);
                normals.push([0.0, -1.0, 0.0]);
                uvs.push([0.5 + 0.5 * cos_a, 0.5 + 0.5 * sin_a]);
                positions.push([inner_bot_r * cos_a, -0.5, inner_bot_r * sin_a]);
                normals.push([0.0, -1.0, 0.0]);
                uvs.push([0.5 + inner_bot_r * cos_a, 0.5 + inner_bot_r * sin_a]);
            }
            for i in 0..arc_steps {
                let b = ann_base + (i * 2) as u32;
                // reversed for downward normal
                add_quad(&mut indices, b, b + 2, b + 3, b + 1);
            }
        }
    }

    // ── Cut caps (only when path is not a full circle) ────────────────────────
    if !is_full {
        // Start cap at begin_a — normal = [sin(a), 0, -cos(a)]
        push_cut_cap(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
            begin_a,
            top_r,
            bot_r,
            inner_top_r,
            inner_bot_r,
            is_hollow,
            true,
        );
        // End cap at end_a — normal = [-sin(a), 0, cos(a)]
        push_cut_cap(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
            end_a,
            top_r,
            bot_r,
            inner_top_r,
            inner_bot_r,
            is_hollow,
            false,
        );
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Push one cut-face quad (or triangle if top_r≈0) at a given angle.
fn push_cut_cap(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    angle: f32,
    top_r: f32,
    bot_r: f32,
    inner_top_r: f32,
    inner_bot_r: f32,
    is_hollow: bool,
    is_start: bool,
) {
    let (cos_a, sin_a) = (angle.cos(), angle.sin());
    let (nx, nz) = if is_start {
        (sin_a, -cos_a)
    } else {
        (-sin_a, cos_a)
    };
    let n = [nx, 0.0, nz];

    let base = positions.len() as u32;

    // 4 vertices: outer_top, outer_bot, inner_top, inner_bot
    // (inner = center axis when not hollow)
    let (it_x, it_z) = if is_hollow {
        (inner_top_r * cos_a, inner_top_r * sin_a)
    } else {
        (0.0, 0.0)
    };
    let (ib_x, ib_z) = if is_hollow {
        (inner_bot_r * cos_a, inner_bot_r * sin_a)
    } else {
        (0.0, 0.0)
    };
    let (ot_x, ot_z) = (top_r * cos_a, top_r * sin_a);
    let (ob_x, ob_z) = (bot_r * cos_a, bot_r * sin_a);

    // outer_top (0)
    positions.push([ot_x, 0.5, ot_z]);
    normals.push(n);
    uvs.push([1.0, 0.0]);
    // outer_bot (1)
    positions.push([ob_x, -0.5, ob_z]);
    normals.push(n);
    uvs.push([1.0, 1.0]);
    // inner_bot (2)
    positions.push([ib_x, -0.5, ib_z]);
    normals.push(n);
    uvs.push([0.0, 1.0]);
    // inner_top (3)
    positions.push([it_x, 0.5, it_z]);
    normals.push(n);
    uvs.push([0.0, 0.0]);

    // Winding verified for start cap: add_quad(outer_top, outer_bot, inner_bot, inner_top)
    // For end cap (reversed direction): add_quad(inner_top, inner_bot, outer_bot, outer_top)
    if is_start {
        // [base, base+1, base+2] and [base, base+2, base+3]
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    } else {
        // [base+3, base+2, base+1] and [base+3, base+1, base]
        indices.extend_from_slice(&[base + 3, base + 2, base + 1, base + 3, base + 1, base]);
    }
}

/// Box mesh with optional angular path cut (a wedge sliced through the vertical
/// axis) and hollow (an inner square shell). The square cross-section is clipped
/// to the angular sector `[begin, end]` (turns, 0..1) measured CCW around the Y
/// axis — the same path-cut convention used for the cylinder.
fn build_box_mesh(path_cut_begin: f32, path_cut_end: f32, hollow: f32) -> Mesh {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

    let is_hollow = hollow > 0.001;
    let begin_a = path_cut_begin * TAU;
    let end_a = path_cut_end * TAU;
    let sweep = (end_a - begin_a).max(0.0);
    let is_full = sweep >= TAU - 0.001;

    let outer_half = 0.5_f32;
    let inner_half = outer_half * hollow;

    // Point where a ray at `angle` from the origin hits a square of the given
    // half-extent, in the XZ plane.
    let square_pt = |angle: f32, half: f32| -> Vec3 {
        let (s, c) = angle.sin_cos();
        let m = c.abs().max(s.abs()).max(1e-6);
        Vec3::new(half * c / m, 0.0, half * s / m)
    };

    // Sample angles: the cut start/end plus every square corner strictly inside
    // the sweep. Corners sit on the diagonals (PI/4 + k·PI/2); splitting there
    // keeps each wall segment flat on a single face.
    let mut angles: Vec<f32> = vec![begin_a];
    for k in 0..4 {
        let corner = FRAC_PI_4 + k as f32 * FRAC_PI_2;
        if corner > begin_a + 1e-4 && corner < end_a - 1e-4 {
            angles.push(corner);
        }
    }
    angles.push(end_a);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let (y_top, y_bot) = (0.5_f32, -0.5_f32);

    // Outward normal of the square edge that a wall segment lies on.
    let face_normal = |mid: Vec3| -> Vec3 {
        if mid.x.abs() >= mid.z.abs() {
            Vec3::new(mid.x.signum(), 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, mid.z.signum())
        }
    };

    // One wall + cap pair per consecutive angle segment.
    for w in angles.windows(2) {
        let (a0, a1) = (w[0], w[1]);
        let o0 = square_pt(a0, outer_half);
        let o1 = square_pt(a1, outer_half);
        let ot0 = Vec3::new(o0.x, y_top, o0.z);
        let ot1 = Vec3::new(o1.x, y_top, o1.z);
        let ob0 = Vec3::new(o0.x, y_bot, o0.z);
        let ob1 = Vec3::new(o1.x, y_bot, o1.z);

        let n_out = face_normal((o0 + o1) * 0.5);
        push_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [ot0, ot1, ob1, ob0], n_out);

        if is_hollow {
            let i0 = square_pt(a0, inner_half);
            let i1 = square_pt(a1, inner_half);
            let it0 = Vec3::new(i0.x, y_top, i0.z);
            let it1 = Vec3::new(i1.x, y_top, i1.z);
            let ib0 = Vec3::new(i0.x, y_bot, i0.z);
            let ib1 = Vec3::new(i1.x, y_bot, i1.z);

            // Inner wall (normal points inward) + top/bottom rings.
            push_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
                [it0, it1, ib1, ib0], -n_out);
            push_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
                [ot0, it0, it1, ot1], Vec3::Y);
            push_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
                [ob0, ib0, ib1, ob1], Vec3::NEG_Y);
        } else {
            // Solid caps: fan from the central axis.
            let ct = Vec3::new(0.0, y_top, 0.0);
            let cb = Vec3::new(0.0, y_bot, 0.0);
            push_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
                [ct, ot0, ot1], Vec3::Y);
            push_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
                [cb, ob0, ob1], Vec3::NEG_Y);
        }
    }

    // Cut faces close the wedge along the begin/end meridians.
    if !is_full {
        for (angle, sign) in [(begin_a, 1.0_f32), (end_a, -1.0_f32)] {
            let (s, c) = angle.sin_cos();
            let n = Vec3::new(s * sign, 0.0, -c * sign);
            let o = square_pt(angle, outer_half);
            let ot = Vec3::new(o.x, y_top, o.z);
            let ob = Vec3::new(o.x, y_bot, o.z);
            if is_hollow {
                let i = square_pt(angle, inner_half);
                let it = Vec3::new(i.x, y_top, i.z);
                let ib = Vec3::new(i.x, y_bot, i.z);
                push_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
                    [ot, ob, ib, it], n);
            } else {
                let ct = Vec3::new(0.0, y_top, 0.0);
                let cb = Vec3::new(0.0, y_bot, 0.0);
                push_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
                    [ot, ob, cb, ct], n);
            }
        }
    }

    finish_mesh(positions, normals, uvs, indices)
}

/// UV-sphere mesh with optional angular path cut (a longitudinal wedge) and
/// hollow (an inner sphere shell). Longitude is swept over `[begin, end]` turns
/// CCW about the Y axis; latitude always spans pole to pole.
fn build_sphere_mesh(path_cut_begin: f32, path_cut_end: f32, hollow: f32) -> Mesh {
    use std::f32::consts::{PI, TAU};

    let stacks: usize = 16; // latitude divisions
    let r = 0.5_f32;
    let inner_r = r * hollow;
    let is_hollow = hollow > 0.001;

    let begin_a = path_cut_begin * TAU;
    let end_a = path_cut_end * TAU;
    let sweep = (end_a - begin_a).max(0.0);
    let is_full = sweep >= TAU - 0.001;
    let slices = if is_full {
        32
    } else {
        ((sweep / TAU * 32.0).round() as usize).max(1)
    };

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Build one sphere shell over the swept region (outward or inward facing).
    let surface = |positions: &mut Vec<[f32; 3]>,
                       normals: &mut Vec<[f32; 3]>,
                       uvs: &mut Vec<[f32; 2]>,
                       indices: &mut Vec<u32>,
                       rad: f32,
                       outward: bool| {
        let base = positions.len() as u32;
        for i in 0..=stacks {
            let v = i as f32 / stacks as f32;
            let lat = v * PI; // 0 (north) .. PI (south)
            let (slat, clat) = lat.sin_cos();
            for j in 0..=slices {
                let t = j as f32 / slices as f32;
                let lon = begin_a + t * sweep;
                let (slon, clon) = lon.sin_cos();
                let dir = Vec3::new(slat * clon, clat, slat * slon);
                positions.push((dir * rad).to_array());
                normals.push(if outward { dir } else { -dir }.to_array());
                uvs.push([t, v]);
            }
        }
        let row = (slices + 1) as u32;
        for i in 0..stacks as u32 {
            for j in 0..slices as u32 {
                let v00 = base + i * row + j;
                let v01 = v00 + 1;
                let v10 = v00 + row;
                let v11 = v10 + 1;
                if outward {
                    indices.extend_from_slice(&[v00, v01, v11, v00, v11, v10]);
                } else {
                    indices.extend_from_slice(&[v00, v11, v01, v00, v10, v11]);
                }
            }
        }
    };

    surface(&mut positions, &mut normals, &mut uvs, &mut indices, r, true);
    if is_hollow {
        surface(&mut positions, &mut normals, &mut uvs, &mut indices, inner_r, false);
    }

    // Cut faces: a meridian half-disc (or half-annulus when hollow) at each end.
    if !is_full {
        for (angle, sign) in [(begin_a, 1.0_f32), (end_a, -1.0_f32)] {
            let (s, c) = angle.sin_cos();
            let n = Vec3::new(s * sign, 0.0, -c * sign);
            let merid = |rad: f32, i: usize| -> Vec3 {
                let lat = i as f32 / stacks as f32 * PI;
                let (slat, clat) = lat.sin_cos();
                Vec3::new(slat * c, clat, slat * s) * rad
            };
            for i in 0..stacks {
                let o0 = merid(r, i);
                let o1 = merid(r, i + 1);
                let (inner0, inner1) = if is_hollow {
                    (merid(inner_r, i), merid(inner_r, i + 1))
                } else {
                    // Collapse to the central axis at the matching latitude.
                    (Vec3::new(0.0, o0.y, 0.0), Vec3::new(0.0, o1.y, 0.0))
                };
                push_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
                    [o0, o1, inner1, inner0], n);
            }
        }
    }

    finish_mesh(positions, normals, uvs, indices)
}

/// Push a flat quad (4 coplanar verts) auto-wound so its front face faces `n`.
fn push_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    verts: [Vec3; 4],
    n: Vec3,
) {
    let facing = (verts[1] - verts[0]).cross(verts[2] - verts[0]).dot(n) >= 0.0;
    let order = if facing { [0, 1, 2, 3] } else { [0, 3, 2, 1] };
    let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let base = positions.len() as u32;
    for &k in &order {
        positions.push(verts[k].to_array());
        normals.push(n.to_array());
        uvs.push(uv[k]);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Push a flat triangle auto-wound so its front face faces `n`.
fn push_tri(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    verts: [Vec3; 3],
    n: Vec3,
) {
    let facing = (verts[1] - verts[0]).cross(verts[2] - verts[0]).dot(n) >= 0.0;
    let order = if facing { [0, 1, 2] } else { [0, 2, 1] };
    let uv = [[0.5, 0.0], [0.0, 1.0], [1.0, 1.0]];
    let base = positions.len() as u32;
    for &k in &order {
        positions.push(verts[k].to_array());
        normals.push(n.to_array());
        uvs.push(uv[k]);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2]);
}

fn finish_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
