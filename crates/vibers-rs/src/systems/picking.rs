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
    // Get the primary window and camera
    let window = match windows.iter().next() {
        Some(w) => w,
        None => return,
    };

    let cursor_position = match window.cursor_position() {
        Some(pos) => pos,
        None => return,
    };

    let (_camera, camera_global_transform) = match cameras.iter().next() {
        Some(cam) => cam,
        None => return,
    };

    // Simplified ray calculation: extend from camera through cursor position
    let viewport_size = Vec2::new(window.width(), window.height());
    let ndc = Vec2::new(
        (cursor_position.x / viewport_size.x) * 2.0 - 1.0,
        1.0 - (cursor_position.y / viewport_size.y) * 2.0,
    );

    // Convert NDC to world space - use a simplified approach
    // Assume orthographic for simplicity, or use the forward direction with some scaling
    let ray_origin = camera_global_transform.translation();
    let camera_forward = camera_global_transform.forward();
    let camera_right = camera_global_transform.right();
    let camera_up = camera_global_transform.up();

    // Adjust ray direction based on cursor position in viewport
    // Convert Dir3 to Vec3 and combine
    let ray_direction = (camera_forward.as_vec3() * 1.0
        + camera_right.as_vec3() * ndc.x * 0.1
        + camera_up.as_vec3() * ndc.y * 0.1)
        .normalize();

    // Simple AABB raycast against prims
    let mut hit_prim: Option<(Entity, &Prim, f32)> = None;
    for (entity, prim, prim_transform) in prim_query.iter() {
        // Simple sphere test (approximate bounding sphere from scale)
        let radius = prim_transform.scale().length() / 2.0;
        let sphere_center = prim_transform.translation();

        if raycast_sphere(ray_origin, ray_direction, sphere_center, radius) {
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

    // Check for left-click (select prim)
    if mouse_buttons.just_pressed(MouseButton::Left) {
        // Deselect previous prim
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

    // Check for right-click (show context menu)
    if mouse_buttons.just_pressed(MouseButton::Right) {
        context_menu.visible = true;
        context_menu.screen_pos = cursor_position;

        if let Some((_, prim, _)) = hit_prim {
            context_menu.hit_prim_id = Some(prim.id);
            context_menu.hit_region_id = Some(prim.region_id);
            context_menu.hit_point = ray_origin + ray_direction * 10.0;
            tracing::debug!(prim_id = prim.id, "context menu for prim");
        } else {
            // Check if clicked on a region for "create prim" option
            context_menu.hit_prim_id = None;

            // Raycast against regions to get hit point
            for (_, region, _) in region_query.iter() {
                context_menu.hit_region_id = Some(region.id);
                context_menu.hit_point = ray_origin + ray_direction * 10.0; // Approximate hit point
                break; // Just use the first region for simplicity
            }

            if context_menu.hit_region_id.is_none() {
                context_menu.visible = false;
            }
            tracing::debug!("context menu for region");
        }
    }
}

/// Simple ray-sphere intersection test
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
        // Brighten the material to show selection
        if let Some(material) = materials.get_mut(&material_handle.0) {
            let brightened = prim.color.to_linear();
            let brightened_srgb = Color::linear_rgba(
                (brightened.red * 1.5).min(1.0),
                (brightened.green * 1.5).min(1.0),
                (brightened.blue * 1.5).min(1.0),
                brightened.alpha,
            );
            material.base_color = brightened_srgb;
            material.emissive = LinearRgba::new(0.2, 0.3, 0.5, 1.0); // Slight emissive glow
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
