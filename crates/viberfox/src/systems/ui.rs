//! Prim UI system (Phase 5 - ADR-017).
//!
//! Renders context menus and edit dialogs using egui.

use super::egui_manager::EguiManager;
use crate::components::{Prim, PrimShape, Region, Selected};
use crate::resources::{ContextMenuState, Database, EditDialogState, GameState};
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use egui::Window;

/// Render context menu when a prim is right-clicked
pub fn render_context_menu(
    mut commands: Commands,
    mut egui: ResMut<EguiManager>,
    mut context_menu: ResMut<ContextMenuState>,
    mut edit_dialog: ResMut<EditDialogState>,
    mut game_state: ResMut<GameState>,
    prim_query: Query<(Entity, &Prim, &Transform)>,
    selected_query: Query<Entity, With<Selected>>,
) {
    if !context_menu.visible {
        return;
    }

    let ctx = egui.ctx_mut();
    let mut menu_open = true;

    // Position the context menu at the mouse click location
    let menu_pos = egui::pos2(context_menu.screen_pos.x, context_menu.screen_pos.y);

    Window::new("Context Menu")
        .open(&mut menu_open)
        .fixed_pos(menu_pos)
        .show(ctx, |ui| {
            if let Some(prim_id) = context_menu.hit_prim_id {
                // Clicked on an existing prim
                if ui.button("Edit Prim (E)").clicked() {
                    for entity in selected_query.iter() {
                        commands.entity(entity).remove::<Selected>();
                    }
                    for (entity, prim, transform) in prim_query.iter() {
                        if prim.id == prim_id {
                            commands.entity(entity).insert(Selected);
                            game_state.selected_prim_id = Some(prim_id);
                            game_state.editing_prim_id = Some(prim_id);
                            edit_dialog.prim_id = Some(prim_id);
                            edit_dialog.is_new = false;
                            edit_dialog.name = prim.name.clone();
                            let color = {
                                let c = prim.color.to_linear();
                                [c.red, c.green, c.blue]
                            };
                            edit_dialog.color = color;
                            let shape = format!("{:?}", prim.shape).to_lowercase();
                            edit_dialog.shape = shape.clone();
                            let pos = transform.translation;
                            edit_dialog.position = [pos.x, pos.y, pos.z];
                            let rot = transform.rotation.to_euler(EulerRot::XYZ);
                            edit_dialog.rotation = [rot.0, rot.1, rot.2];
                            let scale_v = transform.scale;
                            edit_dialog.scale = [scale_v.x, scale_v.y, scale_v.z];
                            // Snapshot for Cancel revert.
                            edit_dialog.original_name = prim.name.clone();
                            edit_dialog.original_color = color;
                            edit_dialog.original_shape = shape;
                            edit_dialog.original_position = edit_dialog.position;
                            edit_dialog.original_rotation = edit_dialog.rotation;
                            edit_dialog.original_scale = edit_dialog.scale;
                            edit_dialog.visible = true;
                            break;
                        }
                    }
                    context_menu.visible = false;
                }

                if ui.button("Delete Prim (D)").clicked() {
                    tracing::info!(prim_id, "deleting prim");
                    game_state.prim_to_delete = Some(prim_id);
                    context_menu.visible = false;
                }
            } else {
                // Clicked on empty space - create new prim
                if ui.button("Create Prim (C)").clicked() {
                    edit_dialog.prim_id = None;
                    edit_dialog.is_new = true;
                    edit_dialog.name = "Prim".to_string();
                    edit_dialog.color = [0.5, 0.5, 0.5]; // neutral gray
                    edit_dialog.shape = "box".to_string();
                    let hit = context_menu.hit_point;
                    // Offset Y so the prim rests on the surface instead of being centred in it.
                    edit_dialog.position = [hit.x, hit.y + 0.5, hit.z];
                    edit_dialog.rotation = [0.0, 0.0, 0.0];
                    edit_dialog.scale = [1.0, 1.0, 1.0];
                    edit_dialog.visible = true;
                    game_state.editing_prim_id = None;
                    context_menu.visible = false;
                }
            }

            if ui.button("Cancel (ESC)").clicked() {
                context_menu.visible = false;
            }
        });

    if !menu_open {
        context_menu.visible = false;
    }
}

/// Render the prim edit dialog
pub fn render_edit_dialog(
    mut egui: ResMut<EguiManager>,
    mut edit_dialog: ResMut<EditDialogState>,
    mut game_state: ResMut<GameState>,
) {
    if !edit_dialog.visible {
        return;
    }

    let ctx = egui.ctx_mut();
    let mut dialog_open = true;

    let title = if edit_dialog.is_new {
        "Create Prim"
    } else {
        "Edit Prim"
    };

    Window::new(title)
        .open(&mut dialog_open)
        .default_width(300.0)
        .show(ctx, |ui| {
            // Prim name field
            ui.label("Name:");
            ui.text_edit_singleline(&mut edit_dialog.name);

            ui.separator();

            // Shape selector
            ui.label("Shape:");
            let shapes = vec!["box", "sphere", "cylinder", "cone"];

            egui::ComboBox::from_label("")
                .selected_text(&edit_dialog.shape)
                .show_ui(ui, |ui| {
                    for shape in shapes {
                        ui.selectable_value(&mut edit_dialog.shape, shape.to_string(), shape);
                    }
                });

            ui.separator();

            // Position fields (Bevy is Y-up: Y is vertical, X/Z are the ground plane)
            ui.label("Position:");
            ui.horizontal(|ui| {
                ui.label("X:");
                ui.add(egui::DragValue::new(&mut edit_dialog.position[0]).speed(0.1));
                ui.label("Y (up):");
                ui.add(egui::DragValue::new(&mut edit_dialog.position[1]).speed(0.1));
                ui.label("Z:");
                ui.add(egui::DragValue::new(&mut edit_dialog.position[2]).speed(0.1));
            });

            // Rotation fields
            ui.label("Rotation (degrees):");
            ui.horizontal(|ui| {
                ui.label("X:");
                let mut rot_x = edit_dialog.rotation[0].to_degrees();
                if ui
                    .add(egui::DragValue::new(&mut rot_x).speed(1.0))
                    .changed()
                {
                    edit_dialog.rotation[0] = rot_x.to_radians();
                }
                ui.label("Y:");
                let mut rot_y = edit_dialog.rotation[1].to_degrees();
                if ui
                    .add(egui::DragValue::new(&mut rot_y).speed(1.0))
                    .changed()
                {
                    edit_dialog.rotation[1] = rot_y.to_radians();
                }
                ui.label("Z:");
                let mut rot_z = edit_dialog.rotation[2].to_degrees();
                if ui
                    .add(egui::DragValue::new(&mut rot_z).speed(1.0))
                    .changed()
                {
                    edit_dialog.rotation[2] = rot_z.to_radians();
                }
            });

            // Scale fields
            ui.label("Scale:");
            ui.horizontal(|ui| {
                ui.label("X:");
                ui.add(egui::DragValue::new(&mut edit_dialog.scale[0]).speed(0.1));
                ui.label("Y:");
                ui.add(egui::DragValue::new(&mut edit_dialog.scale[1]).speed(0.1));
                ui.label("Z:");
                ui.add(egui::DragValue::new(&mut edit_dialog.scale[2]).speed(0.1));
            });

            ui.separator();

            // Color picker
            ui.label("Color:");
            let mut srgb = [
                (edit_dialog.color[0] * 255.0) as u8,
                (edit_dialog.color[1] * 255.0) as u8,
                (edit_dialog.color[2] * 255.0) as u8,
            ];
            if ui.color_edit_button_srgb(&mut srgb).changed() {
                edit_dialog.color = [
                    srgb[0] as f32 / 255.0,
                    srgb[1] as f32 / 255.0,
                    srgb[2] as f32 / 255.0,
                ];
            }

            ui.separator();

            // Dialog buttons
            ui.horizontal(|ui| {
                if ui.button("Save (S)").clicked() {
                    game_state.pending_prim_save = Some(edit_dialog.clone());
                    edit_dialog.visible = false;
                    game_state.editing_prim_id = None;
                }

                if ui.button("Cancel (ESC)").clicked() {
                    push_revert(&mut game_state, &edit_dialog);
                    edit_dialog.visible = false;
                    game_state.editing_prim_id = None;
                }

                if !edit_dialog.is_new && ui.button("Delete (D)").clicked() {
                    game_state.prim_to_delete = edit_dialog.prim_id;
                    edit_dialog.visible = false;
                    game_state.editing_prim_id = None;
                }
            });
        });

    if !dialog_open {
        push_revert(&mut game_state, &edit_dialog);
        edit_dialog.visible = false;
        game_state.editing_prim_id = None;
    }
}

/// Sync dialog values → prim entity every frame while the edit dialog is open.
/// This gives live preview; Save persists to DB, Cancel reverts via `pending_prim_revert`.
pub fn apply_live_prim_edits(
    edit_dialog: Res<EditDialogState>,
    game_state: Res<GameState>,
    mut prim_query: Query<(
        &mut Prim,
        &mut Transform,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !edit_dialog.visible || edit_dialog.is_new {
        return;
    }
    let Some(prim_id) = game_state.editing_prim_id else {
        return;
    };
    for (mut prim, mut transform, mat_handle) in prim_query.iter_mut() {
        if prim.id != prim_id {
            continue;
        }
        transform.translation = Vec3::new(
            edit_dialog.position[0],
            edit_dialog.position[1],
            edit_dialog.position[2],
        );
        transform.rotation = Quat::from_euler(
            EulerRot::XYZ,
            edit_dialog.rotation[0],
            edit_dialog.rotation[1],
            edit_dialog.rotation[2],
        );
        transform.scale = Vec3::new(
            edit_dialog.scale[0],
            edit_dialog.scale[1],
            edit_dialog.scale[2],
        );
        let new_color =
            Color::srgb(edit_dialog.color[0], edit_dialog.color[1], edit_dialog.color[2]);
        prim.color = new_color;
        prim.name = edit_dialog.name.clone();
        // Update material directly — Selected doesn't change during editing so
        // highlight_selected_prim won't fire on its own.
        if let Some(handle) = mat_handle {
            if let Some(mat) = materials.get_mut(&handle.0) {
                let lin = new_color.to_linear();
                mat.base_color = Color::linear_rgba(
                    (lin.red * 1.5).min(1.0),
                    (lin.green * 1.5).min(1.0),
                    (lin.blue * 1.5).min(1.0),
                    lin.alpha,
                );
            }
        }
        break;
    }
}

fn push_revert(game_state: &mut GameState, dialog: &EditDialogState) {
    if !dialog.is_new {
        use crate::resources::EditDialogState;
        game_state.pending_prim_revert = Some(EditDialogState {
            prim_id: dialog.prim_id,
            name: dialog.original_name.clone(),
            shape: dialog.original_shape.clone(),
            position: dialog.original_position,
            rotation: dialog.original_rotation,
            scale: dialog.original_scale,
            color: dialog.original_color,
            ..Default::default()
        });
    }
}

/// Persist prim changes to the local database and update the Bevy world.
pub fn send_prim_mutations(
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
    db: Option<Res<Database>>,
    mut prim_query: Query<(Entity, &mut Prim, &mut Transform)>,
    region_query: Query<&Region>,
) {
    if let Some(dialog_state) = game_state.pending_prim_save.take() {
        if dialog_state.is_new {
            let region_id = region_query.iter().next().map(|r| r.id).unwrap_or(1);

            let new_id: Option<i64> = db.as_ref().and_then(|db| {
                let conn = db.conn.lock().unwrap();
                conn.execute(
                    "INSERT INTO prims \
                     (region_id, name, shape, \
                      position_x, position_y, position_z, \
                      rotation_x, rotation_y, rotation_z, \
                      scale_x, scale_y, scale_z, \
                      color_r, color_g, color_b, \
                      created_at, updated_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,\
                             datetime('now'),datetime('now'))",
                    rusqlite::params![
                        region_id,
                        dialog_state.name,
                        dialog_state.shape,
                        dialog_state.position[0] as f64,
                        dialog_state.position[1] as f64,
                        dialog_state.position[2] as f64,
                        dialog_state.rotation[0] as f64,
                        dialog_state.rotation[1] as f64,
                        dialog_state.rotation[2] as f64,
                        dialog_state.scale[0] as f64,
                        dialog_state.scale[1] as f64,
                        dialog_state.scale[2] as f64,
                        dialog_state.color[0] as f64,
                        dialog_state.color[1] as f64,
                        dialog_state.color[2] as f64,
                    ],
                )
                .ok()?;
                Some(conn.last_insert_rowid())
            });

            if let Some(id) = new_id {
                commands.spawn((
                    Prim {
                        id,
                        region_id,
                        name: dialog_state.name.clone(),
                        shape: PrimShape::from_str(&dialog_state.shape),
                        color: Color::srgb(
                            dialog_state.color[0],
                            dialog_state.color[1],
                            dialog_state.color[2],
                        ),
                    },
                    Transform::from_xyz(
                        dialog_state.position[0],
                        dialog_state.position[1],
                        dialog_state.position[2],
                    )
                    .with_rotation(Quat::from_euler(
                        EulerRot::XYZ,
                        dialog_state.rotation[0],
                        dialog_state.rotation[1],
                        dialog_state.rotation[2],
                    ))
                    .with_scale(Vec3::new(
                        dialog_state.scale[0],
                        dialog_state.scale[1],
                        dialog_state.scale[2],
                    )),
                    Selected,
                ));
                game_state.selected_prim_id = Some(id);
                tracing::info!(id, name = %dialog_state.name, "created prim");
            }
        } else if let Some(prim_id) = dialog_state.prim_id {
            if let Some(ref db) = db {
                let conn = db.conn.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE prims SET name=?1, shape=?2, \
                     position_x=?3, position_y=?4, position_z=?5, \
                     rotation_x=?6, rotation_y=?7, rotation_z=?8, \
                     scale_x=?9, scale_y=?10, scale_z=?11, \
                     color_r=?12, color_g=?13, color_b=?14, \
                     updated_at=datetime('now') WHERE id=?15",
                    rusqlite::params![
                        dialog_state.name,
                        dialog_state.shape,
                        dialog_state.position[0] as f64,
                        dialog_state.position[1] as f64,
                        dialog_state.position[2] as f64,
                        dialog_state.rotation[0] as f64,
                        dialog_state.rotation[1] as f64,
                        dialog_state.rotation[2] as f64,
                        dialog_state.scale[0] as f64,
                        dialog_state.scale[1] as f64,
                        dialog_state.scale[2] as f64,
                        dialog_state.color[0] as f64,
                        dialog_state.color[1] as f64,
                        dialog_state.color[2] as f64,
                        prim_id,
                    ],
                );
            }
            for (entity, mut prim, mut transform) in prim_query.iter_mut() {
                if prim.id == prim_id {
                    prim.name = dialog_state.name.clone();
                    prim.shape = PrimShape::from_str(&dialog_state.shape);
                    prim.color = Color::srgb(
                        dialog_state.color[0],
                        dialog_state.color[1],
                        dialog_state.color[2],
                    );
                    *transform = Transform::from_xyz(
                        dialog_state.position[0],
                        dialog_state.position[1],
                        dialog_state.position[2],
                    )
                    .with_rotation(Quat::from_euler(
                        EulerRot::XYZ,
                        dialog_state.rotation[0],
                        dialog_state.rotation[1],
                        dialog_state.rotation[2],
                    ))
                    .with_scale(Vec3::new(
                        dialog_state.scale[0],
                        dialog_state.scale[1],
                        dialog_state.scale[2],
                    ));
                    commands.entity(entity).insert(Selected);
                    game_state.selected_prim_id = Some(prim_id);
                    tracing::info!(id = prim_id, "updated prim");
                    break;
                }
            }
        }
    }

    if let Some(revert) = game_state.pending_prim_revert.take() {
        if let Some(prim_id) = revert.prim_id {
            for (entity, mut prim, mut transform) in prim_query.iter_mut() {
                if prim.id == prim_id {
                    prim.name = revert.name.clone();
                    prim.shape = PrimShape::from_str(&revert.shape);
                    prim.color = Color::srgb(revert.color[0], revert.color[1], revert.color[2]);
                    *transform = Transform::from_xyz(
                        revert.position[0],
                        revert.position[1],
                        revert.position[2],
                    )
                    .with_rotation(Quat::from_euler(
                        EulerRot::XYZ,
                        revert.rotation[0],
                        revert.rotation[1],
                        revert.rotation[2],
                    ))
                    .with_scale(Vec3::new(revert.scale[0], revert.scale[1], revert.scale[2]));
                    // Re-insert Selected so highlight_selected_prim refreshes the material.
                    commands.entity(entity).insert(Selected);
                    game_state.selected_prim_id = Some(prim_id);
                    break;
                }
            }
        }
    }

    if let Some(prim_id) = game_state.prim_to_delete.take() {
        if let Some(ref db) = db {
            let conn = db.conn.lock().unwrap();
            let _ = conn.execute(
                "DELETE FROM prims WHERE id=?1",
                rusqlite::params![prim_id],
            );
        }
        for (entity, prim, _) in prim_query.iter() {
            if prim.id == prim_id {
                commands.entity(entity).despawn();
                tracing::info!(id = prim_id, "deleted prim");
                break;
            }
        }
    }
}
