use crate::components::{Prim, PrimShape, Region};
use crate::systems::tile_loader::{RegionTile, TileKey};
use bevy::math::primitives::{Cuboid, Cylinder, Sphere, Torus};
use bevy::prelude::*;
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
) {
    for (entity, prim, transform) in prim_query.iter() {
        // Unit meshes — Transform.scale on the entity handles the actual size in metres.
        let mesh_handle = match prim.shape {
            PrimShape::Box => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            PrimShape::Sphere => meshes.add(Sphere::new(0.5)),
            PrimShape::Cylinder => meshes.add(Cylinder::new(0.5, 1.0)),
            PrimShape::Cone => meshes.add(Cylinder::new(0.0, 1.0)),
            PrimShape::Torus => meshes.add(Torus::default()),
        };

        let material_handle = materials.add(StandardMaterial {
            base_color: prim.color,
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
