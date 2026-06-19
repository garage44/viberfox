use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy_atmosphere::plugin::AtmosphereSkyBox;
use bevy_atmosphere::prelude::*;
use bevy_atmosphere::skybox::{self, AtmosphereSkyBoxMaterial};
use big_space::prelude::{BigSpaceCommands, FloatingOrigin, Grid};

use crate::resources::{AvatarState, CameraState, CameraMode};
use crate::systems::egui_manager::EguiManager;
use crate::systems::gizmo::GizmoState;
use crate::systems::rendering::RegionMesh;

#[derive(Component)]
pub struct FreeCamera;

// Simple constants
const FREE_CAMERA_SPEED: f32 = 45.0;
const FREE_CAMERA_SPEED_FAST: f32 = 180.0;
const MOUSE_SENSITIVITY: f32 = 0.002;
const MIN_CAMERA_HEIGHT: f32 = 0.5;

/// Match [`bevy_atmosphere::settings::SkyboxCreationMode`] fallback when projection far is unavailable.
const SKYBOX_MESH_FAR: f32 = 1000.0;

/// Grid cell edge for the world `BigSpace` (ADR-019/021). Large enough that local
/// play rarely recenters; small enough to keep within-cell f32 precise.
pub const WORLD_CELL_EDGE: f32 = 10_000.0;
/// Hysteresis buffer past a cell edge before big_space recenters an entity.
pub const WORLD_SWITCH_THRESHOLD: f32 = 100.0;

/// Spawn the world [`BigSpace`] root with the camera as its [`FloatingOrigin`]
/// (ADR-019). Other content currently spawns at the origin (cell 0) and renders
/// correctly; geo-derived `GridCell` placement arrives with planet-scale streaming.
pub fn setup_camera(
    mut commands: Commands,
    mut camera_state: ResMut<CameraState>,
    sky_material: Res<AtmosphereSkyBoxMaterial>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    camera_state.mode = CameraMode::Avatar;
    camera_state.distance = 5.0;
    camera_state.azimuth = 0.0;
    camera_state.pitch = std::f32::consts::PI / 6.0;

    let skybox_mesh = meshes.add(skybox::mesh(SKYBOX_MESH_FAR));
    let sky_handle = sky_material.0.clone();

    commands.spawn_big_space(
        Grid::new(WORLD_CELL_EDGE, WORLD_SWITCH_THRESHOLD),
        |root| {
            root.spawn_spatial((
                Camera3d::default(),
                Transform::from_xyz(0.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
                AtmosphereCamera::default(),
                FreeCamera,
                FloatingOrigin,
            ))
            .with_child((
                Mesh3d(skybox_mesh),
                MeshMaterial3d(sky_handle),
                AtmosphereSkyBox,
                NotShadowCaster,
                NotShadowReceiver,
            ));
        },
    );
}

pub fn camera_mode_toggle(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut camera_state: ResMut<CameraState>,
    camera_query: Query<&Transform, With<FreeCamera>>,
) {
    if keyboard_input.just_pressed(KeyCode::Tab) {
        camera_state.mode = match camera_state.mode {
            CameraMode::Avatar => {
                // Initialize free camera rotation from current camera orientation
                if let Ok(camera_transform) = camera_query.single() {
                    let euler = camera_transform.rotation.to_euler(EulerRot::YXZ);
                    camera_state.free_camera_rotation = Vec2::new(euler.1, euler.0);
                }
                CameraMode::Free
            }
            CameraMode::Free => CameraMode::Avatar,
        };
        println!("Camera mode: {:?}", camera_state.mode);
    }
}

pub fn camera_controls(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_wheel_events: EventReader<bevy::input::mouse::MouseWheel>,
    mut camera_query: Query<&mut Transform, With<FreeCamera>>,
    mut camera_state: ResMut<CameraState>,
    avatar_state: Res<AvatarState>,
    time: Res<Time>,
    region_mesh_query: Query<&GlobalTransform, With<RegionMesh>>,
    gizmo_state: Res<GizmoState>,
    egui_manager: Res<EguiManager>,
) {
    if camera_query.is_empty() {
        return;
    }

    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };
    let delta_time = time.delta().as_secs_f32();

    match camera_state.mode {
        CameraMode::Avatar => {
            // Third-person camera following avatar (smoothed when online — see `AvatarState::display_position`)
            let avatar_pos = avatar_state.display_position;

            // Handle mouse wheel zoom
            for event in mouse_wheel_events.read() {
                camera_state.distance += event.y * 0.1;
                camera_state.distance = camera_state.distance.max(2.0).min(100.0);
            }

            // Handle mouse drag for rotation (suppressed while egui, gizmo, or Ctrl has the pointer).
            let ctrl = keyboard_input.pressed(KeyCode::ControlLeft)
                || keyboard_input.pressed(KeyCode::ControlRight);
            if gizmo_state.active_axis.is_some()
                || egui_manager.ctx.wants_pointer_input()
                || ctrl
            {
                camera_state.pan_offset = None;
                cursor_moved_events.clear();
            } else if mouse_input.pressed(MouseButton::Left) {
                for event in cursor_moved_events.read() {
                    if let Some(last_pos) = camera_state.pan_offset {
                        let delta = event.position - last_pos;
                        camera_state.azimuth -= delta.x * 0.005;
                        camera_state.pitch -= delta.y * 0.005;
                        camera_state.pitch = camera_state.pitch
                            .max(-std::f32::consts::PI / 3.0)
                            .min(std::f32::consts::PI / 2.5);
                    }
                    camera_state.pan_offset = Some(event.position);
                }
            } else if mouse_input.just_released(MouseButton::Left) {
                camera_state.pan_offset = None;
            }

            // Calculate camera position
            let horizontal_distance = camera_state.distance * camera_state.pitch.cos();
            let vertical_offset = camera_state.distance * camera_state.pitch.sin();

            let camera_offset = Vec3::new(
                camera_state.azimuth.sin() * horizontal_distance,
                vertical_offset + 1.5,
                camera_state.azimuth.cos() * horizontal_distance,
            );

            let mut target_position = avatar_pos + camera_offset;

            // Prevent camera from going below ground
            let ground_height = get_ground_height(target_position, &region_mesh_query);
            let min_height = ground_height + MIN_CAMERA_HEIGHT;
            if target_position.y < min_height {
                target_position.y = min_height;
            }

            // Framerate-independent smoothing so orbit matches the follow target
            let cam_alpha = 1.0 - (-12.0_f32 * delta_time).exp();
            camera_transform.translation = camera_transform.translation.lerp(target_position, cam_alpha);
            camera_transform.look_at(avatar_pos + Vec3::Y * 1.5, Vec3::Y);
        }
        CameraMode::Free => {
            // Free camera mode - simple FPS style
            let mut rotation_delta = Vec2::ZERO;

            // Mouse look (left mouse button — right is reserved for context menu).
            // Suppressed while egui, gizmo, or Ctrl has the pointer.
            let ctrl = keyboard_input.pressed(KeyCode::ControlLeft)
                || keyboard_input.pressed(KeyCode::ControlRight);
            if gizmo_state.active_axis.is_some()
                || egui_manager.ctx.wants_pointer_input()
                || ctrl
            {
                camera_state.pan_offset = None;
                cursor_moved_events.clear();
            } else if mouse_input.pressed(MouseButton::Left) {
                for event in cursor_moved_events.read() {
                    if let Some(last_pos) = camera_state.pan_offset {
                        let delta = event.position - last_pos;
                        rotation_delta.x += delta.x;
                        rotation_delta.y += delta.y;
                    }
                    camera_state.pan_offset = Some(event.position);
                }
            } else {
                camera_state.pan_offset = None;
            }

            // Update rotation from mouse input
            if rotation_delta.length() > 0.0 {
                camera_state.free_camera_rotation.y -= rotation_delta.x * MOUSE_SENSITIVITY;
                camera_state.free_camera_rotation.x -= rotation_delta.y * MOUSE_SENSITIVITY;
                camera_state.free_camera_rotation.x = camera_state.free_camera_rotation.x
                    .max(-std::f32::consts::PI / 2.0 + 0.1)
                    .min(std::f32::consts::PI / 2.0 - 0.1);
            }

            // Apply rotation
            let rotation_quat = Quat::from_euler(
                EulerRot::YXZ,
                camera_state.free_camera_rotation.y,
                camera_state.free_camera_rotation.x,
                0.0,
            );
            camera_transform.rotation = rotation_quat;

            // Movement - calculate forward/right from rotation, keep vertical independent
            let forward = rotation_quat * -Vec3::Z;
            let right = rotation_quat * Vec3::X;

            // Flatten to horizontal plane for WASD movement
            let forward_horizontal = Vec3::new(forward.x, 0.0, forward.z).normalize();
            let right_horizontal = Vec3::new(right.x, 0.0, right.z).normalize();

            let speed = if keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight) {
                FREE_CAMERA_SPEED_FAST
            } else {
                FREE_CAMERA_SPEED
            };

            let mut move_direction = Vec3::ZERO;

            if keyboard_input.pressed(KeyCode::KeyW) {
                move_direction += forward_horizontal;
            }
            if keyboard_input.pressed(KeyCode::KeyS) {
                move_direction -= forward_horizontal;
            }
            if keyboard_input.pressed(KeyCode::KeyA) {
                move_direction -= right_horizontal;
            }
            if keyboard_input.pressed(KeyCode::KeyD) {
                move_direction += right_horizontal;
            }

            // Vertical movement - ALWAYS world-space, never affects rotation
            if keyboard_input.pressed(KeyCode::Space) {
                move_direction += Vec3::Y;
            }
            // Q or Shift for moving down (but Shift also speeds up, so only use Shift if not moving horizontally)
            if keyboard_input.pressed(KeyCode::KeyQ) {
                move_direction -= Vec3::Y;
            }
            // Shift alone (without WASD) moves down
            if (keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight))
                && !keyboard_input.pressed(KeyCode::KeyW)
                && !keyboard_input.pressed(KeyCode::KeyA)
                && !keyboard_input.pressed(KeyCode::KeyS)
                && !keyboard_input.pressed(KeyCode::KeyD) {
                move_direction -= Vec3::Y;
            }

            // Suppress keyboard-driven movement while the UI owns the keyboard (text
            // field focused or a Ctrl chord like Ctrl+Shift+A).
            if egui_manager.ui_owns_keyboard(false, &keyboard_input) {
                move_direction = Vec3::ZERO;
            }

            // Apply movement
            if move_direction.length() > 0.0 {
                move_direction = move_direction.normalize();
                camera_transform.translation += move_direction * speed * delta_time;
            }

            // Prevent going below ground
            let ground_height = get_ground_height(camera_transform.translation, &region_mesh_query);
            let min_height = ground_height + MIN_CAMERA_HEIGHT;
            if camera_transform.translation.y < min_height {
                camera_transform.translation.y = min_height;
            }
        }
    }
}

fn get_ground_height(position: Vec3, region_mesh_query: &Query<&GlobalTransform, With<RegionMesh>>) -> f32 {
    let mut min_height: f32 = 0.0;
    for region_transform in region_mesh_query.iter() {
        let region_y: f32 = region_transform.translation().y;
        let region_top: f32 = region_y + 0.05;
        if position.x >= region_transform.translation().x - 150.0
            && position.x <= region_transform.translation().x + 150.0
            && position.z >= region_transform.translation().z - 150.0
            && position.z <= region_transform.translation().z + 150.0 {
            min_height = min_height.max(region_top);
        }
    }
    min_height
}
