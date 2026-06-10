//! Transform gizmo system (Phase 6 - ADR-017).
//!
//! Provides interactive translate/rotate/scale handles for selected prims (simplified version).
//! Gizmo visualization and interaction will be enhanced in future iterations.

use crate::components::{Prim, Selected};
use crate::resources::{EditDialogState, OnlineSession};
use bevy::prelude::*;
use vibe_core::NetMessage;

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

#[derive(Resource, Default)]
pub struct GizmoState {
    pub mode: GizmoMode,
    pub is_dragging: bool,
    pub drag_start_pos: Vec2,
    pub last_update_time: f32,
}

/// Handle gizmo mode switching via keyboard (T=translate, R=rotate, S=scale)
pub fn handle_gizmo_mode_input(
    mut gizmo_state: ResMut<GizmoState>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if keyboard.just_pressed(KeyCode::KeyT) {
        gizmo_state.mode = GizmoMode::Translate;
        tracing::debug!("gizmo mode: translate");
    } else if keyboard.just_pressed(KeyCode::KeyR) {
        gizmo_state.mode = GizmoMode::Rotate;
        tracing::debug!("gizmo mode: rotate");
    } else if keyboard.just_pressed(KeyCode::KeyS) {
        gizmo_state.mode = GizmoMode::Scale;
        tracing::debug!("gizmo mode: scale");
    }
}

/// Render gizmo visualization (simplified version - just debug output)
pub fn render_gizmo_visuals(
    selected_query: Query<&GlobalTransform, With<Selected>>,
    gizmo_state: Res<GizmoState>,
) {
    for transform in selected_query.iter() {
        let pos = transform.translation();
        tracing::debug!(
            ?pos,
            mode = ?gizmo_state.mode,
            "gizmo active"
        );
    }
}

/// Handle mouse-based gizmo interaction
pub fn handle_gizmo_interaction(
    mut selected_query: Query<(&mut Transform, &Prim), With<Selected>>,
    mut edit_dialog: ResMut<EditDialogState>,
    mut gizmo_state: ResMut<GizmoState>,
    windows: Query<&Window>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    online_session: Option<Res<OnlineSession>>,
) {
    if selected_query.is_empty() {
        return;
    }

    let window = match windows.iter().next() {
        Some(w) => w,
        None => return,
    };

    let cursor_pos = match window.cursor_position() {
        Some(pos) => pos,
        None => return,
    };

    // Start drag on left mouse button
    if mouse_buttons.just_pressed(MouseButton::Left) {
        gizmo_state.is_dragging = true;
        gizmo_state.drag_start_pos = cursor_pos;
    }

    if mouse_buttons.just_released(MouseButton::Left) {
        gizmo_state.is_dragging = false;
        // Send update to server when drag finishes
        if let Ok((transform, prim)) = selected_query.single() {
            if let Some(ref session) = online_session {
                gizmo_state.last_update_time = 0.0; // Allow immediate send
                let (roll, pitch, yaw) = transform.rotation.to_euler(EulerRot::XYZ);
                let msg = NetMessage::UpdatePrim {
                    request_id: generate_request_id(),
                    prim_id: prim.id,
                    position: transform.translation,
                    rotation: Vec3::new(roll, pitch, yaw),
                    scale: transform.scale,
                    color: edit_dialog.color,
                    texture_id: edit_dialog.texture_id.clone(),
                    name: edit_dialog.name.clone(),
                };
                let _ = session.intent_tx.send(msg);
                tracing::debug!(prim_id = prim.id, "gizmo update sent");
            }
        }
    }

    // Apply drag updates
    if gizmo_state.is_dragging {
        let delta = cursor_pos - gizmo_state.drag_start_pos;
        let _delta_normalized = delta / 100.0; // Scale for reasonable movement

        if let Ok((mut transform, prim)) = selected_query.single_mut() {
            match gizmo_state.mode {
                GizmoMode::Translate => {
                    // Move along XZ plane based on mouse delta
                    transform.translation.x += delta.x * 0.01;
                    transform.translation.z += delta.y * 0.01;
                }
                GizmoMode::Rotate => {
                    // Rotate based on mouse delta
                    let rot_x = delta.y * 0.01;
                    let rot_y = delta.x * 0.01;
                    transform.rotation =
                        Quat::from_euler(EulerRot::XYZ, rot_x, rot_y, 0.0) * transform.rotation;
                }
                GizmoMode::Scale => {
                    // Scale uniformly based on vertical mouse movement
                    let scale_factor = 1.0 + delta.y * 0.001;
                    transform.scale *= scale_factor;
                }
            }

            // Update edit dialog state
            let pos = transform.translation;
            edit_dialog.position = [pos.x, pos.y, pos.z];
            let (roll, pitch, yaw) = transform.rotation.to_euler(EulerRot::XYZ);
            edit_dialog.rotation = [roll.to_degrees(), pitch.to_degrees(), yaw.to_degrees()];
            let scale = transform.scale;
            edit_dialog.scale = [scale.x, scale.y, scale.z];

            // Throttle server updates to match simulation tick rate (~20 Hz = 50 ms)
            gizmo_state.last_update_time += time.delta_secs();
            if gizmo_state.last_update_time >= 0.05 {
                gizmo_state.last_update_time = 0.0;
                if let Some(ref session) = online_session {
                    let msg = NetMessage::UpdatePrim {
                        request_id: generate_request_id(),
                        prim_id: prim.id,
                        position: transform.translation,
                        rotation: Vec3::new(roll, pitch, yaw),
                        scale: transform.scale,
                        color: edit_dialog.color,
                        texture_id: edit_dialog.texture_id.clone(),
                        name: edit_dialog.name.clone(),
                    };
                    let _ = session.intent_tx.send(msg);
                }
            }
        }
    }
}

/// Generate a simple request ID (should be unique and incrementing in production)
fn generate_request_id() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
