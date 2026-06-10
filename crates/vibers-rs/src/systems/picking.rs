//! Prim selection via raycasting (Phase 4 - ADR-017).
//!
//! On left-click: select prim under cursor, add Selected marker.
//! On right-click: show context menu with options to create/edit/delete.

use crate::components::{Prim, Selected};
use crate::resources::{ContextMenuState, GameState};
use crate::systems::rendering::PrimMesh;
use bevy::prelude::*;

/// System for handling prim picking and selection via raycasting.
pub fn prim_picking(
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
    mut context_menu: ResMut<ContextMenuState>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    prim_query: Query<(Entity, &Prim, &GlobalTransform), With<PrimMesh>>,
    region_query: Query<(Entity, &crate::components::Region, &GlobalTransform), Without<Prim>>,
    selected_query: Query<Entity, With<Selected>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    let window = match windows.iter().next() {
        Some(w) => w,
        None => return,
    };

    let cursor_position = match window.cursor_position() {
        Some(pos) => pos,
        None => return,
    };

    let (camera, camera_global_transform) = match cameras.iter().next() {
        Some(cam) => cam,
        None => return,
    };

    // Proper perspective ray through the cursor position.
    let Ok(ray) = camera.viewport_to_world(camera_global_transform, cursor_position) else {
        return;
    };

    let ray_origin = ray.origin;
    let ray_dir = ray.direction.as_vec3();

    // Sphere-based prim hit test. For the currently selected prim we extend the
    // radius to cover the gizmo arm so clicking on an axis handle does not deselect.
    let mut hit_prim: Option<(Entity, &Prim, f32)> = None;
    for (entity, prim, prim_transform) in prim_query.iter() {
        let max_scale = prim_transform.scale().max_element();
        let radius = if game_state.selected_prim_id == Some(prim.id) {
            (max_scale * 0.8).max(1.2) // cover gizmo arm
        } else {
            max_scale * 0.5
        };
        let sphere_center = prim_transform.translation();

        if raycast_sphere(ray_origin, ray_dir, sphere_center, radius) {
            let distance = (sphere_center - ray_origin).length();
            match hit_prim {
                None => hit_prim = Some((entity, prim, distance)),
                Some((_, _, best_dist)) if distance < best_dist => {
                    hit_prim = Some((entity, prim, distance));
                }
                _ => {}
            }
        }
    }

    // Left-click: select prim.
    if mouse_buttons.just_pressed(MouseButton::Left) {
        for entity in selected_query.iter() {
            commands.entity(entity).remove::<Selected>();
        }

        if let Some((entity, prim, _)) = hit_prim {
            game_state.selected_prim_id = Some(prim.id);
            commands.entity(entity).insert(Selected);
            tracing::debug!(prim_id = prim.id, "prim selected");
        } else {
            game_state.selected_prim_id = None;
            tracing::debug!("prim deselected");
        }
    }

    // Right-click: open context menu.
    if mouse_buttons.just_pressed(MouseButton::Right) {
        context_menu.visible = true;
        context_menu.screen_pos = cursor_position;

        if let Some((_, prim, _)) = hit_prim {
            context_menu.hit_prim_id = Some(prim.id);
            context_menu.hit_region_id = Some(prim.region_id);
            context_menu.hit_point = ray_plane_y(ray_origin, ray_dir, 0.0)
                .unwrap_or_else(|| ray_origin + ray_dir * 10.0);
            tracing::debug!(prim_id = prim.id, "context menu for prim");
        } else {
            context_menu.hit_prim_id = None;

            let mut found_region = false;
            for (_, region, region_transform) in region_query.iter() {
                let region_y = region_transform.translation().y;
                context_menu.hit_region_id = Some(region.id);
                context_menu.hit_point = ray_plane_y(ray_origin, ray_dir, region_y)
                    .unwrap_or_else(|| ray_origin + ray_dir * 20.0);
                found_region = true;
                break;
            }

            if !found_region {
                context_menu.visible = false;
            }
            tracing::debug!("context menu for region");
        }
    }
}

/// Intersect ray with horizontal plane at the given y level.
/// Returns None if the ray is nearly parallel to the plane or points away from it.
fn ray_plane_y(origin: Vec3, dir: Vec3, y: f32) -> Option<Vec3> {
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t = (y - origin.y) / dir.y;
    if t <= 0.0 {
        return None;
    }
    Some(origin + dir * t)
}

/// Simple ray-sphere intersection test.
fn raycast_sphere(ray_origin: Vec3, ray_dir: Vec3, sphere_center: Vec3, radius: f32) -> bool {
    let oc = ray_origin - sphere_center;
    let a = ray_dir.dot(ray_dir);
    let b = 2.0 * oc.dot(ray_dir);
    let c = oc.dot(oc) - radius * radius;
    let discriminant = b * b - 4.0 * a * c;

    discriminant >= 0.0 && {
        let t = (-b - discriminant.sqrt()) / (2.0 * a);
        t > 0.0
    }
}

/// System to highlight selected prim with a different material color
pub fn highlight_selected_prim(
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected_query: Query<
        (&Prim, &MeshMaterial3d<StandardMaterial>),
        (With<Selected>, Changed<Selected>),
    >,
) {
    for (prim, material_handle) in selected_query.iter() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            let brightened = prim.color.to_linear();
            material.base_color = Color::linear_rgba(
                (brightened.red * 1.5).min(1.0),
                (brightened.green * 1.5).min(1.0),
                (brightened.blue * 1.5).min(1.0),
                brightened.alpha,
            );
            material.emissive = LinearRgba::new(0.2, 0.3, 0.5, 1.0);
        }
    }
}

/// System to restore original color when prim is deselected
pub fn unhighlight_deselected_prim(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(&Prim, &MeshMaterial3d<StandardMaterial>), With<Selected>>,
    mut deselected: RemovedComponents<Selected>,
) {
    for entity in deselected.read() {
        if let Ok((prim, material_handle)) = query.get_mut(entity) {
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.base_color = prim.color;
                material.emissive = LinearRgba::BLACK;
            }
        }
    }
}
