//! Prim UI system (Phase 5 - ADR-017).
//!
//! Renders context menus and edit dialogs using egui.

use super::egui_manager::EguiManager;
use crate::components::{Prim, PrimShape, Region, Selected};
use crate::resources::{
    ContextMenuState, Database, EditDialogState, GameState, PrimTextureCache, TextureLibrary,
};
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
                            edit_dialog.texture_id = prim.texture_id.clone();
                            edit_dialog.original_name = prim.name.clone();
                            edit_dialog.original_color = color;
                            edit_dialog.original_shape = shape;
                            edit_dialog.original_position = edit_dialog.position;
                            edit_dialog.original_rotation = edit_dialog.rotation;
                            edit_dialog.original_scale = edit_dialog.scale;
                            edit_dialog.original_texture_id = prim.texture_id.clone();
                            edit_dialog.texture_picker_open = false;
                            edit_dialog.visible = true;
                            break;
                        }
                    }
                    context_menu.visible = false;
                }

                if ui.button("Delete Prim (D)").clicked() {
                    tracing::info!(prim_id, "deleting prim");
                    game_state.prims_to_delete.push(prim_id);
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
                    edit_dialog.texture_id = None;
                    edit_dialog.texture_picker_open = false;
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
    texture_lib: Res<TextureLibrary>,
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

            // Texture picker
            ui.label("Texture:");
            ui.horizontal(|ui| {
                let tex_label = edit_dialog
                    .texture_id
                    .as_deref()
                    .unwrap_or("None");
                ui.label(tex_label);
                if !texture_lib.entries.is_empty() && ui.button("Pick…").clicked() {
                    edit_dialog.texture_picker_open = !edit_dialog.texture_picker_open;
                }
                if edit_dialog.texture_id.is_some() && ui.button("Clear").clicked() {
                    edit_dialog.texture_id = None;
                }
            });

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
                    if let Some(id) = edit_dialog.prim_id {
                        game_state.prims_to_delete.push(id);
                    }
                    edit_dialog.visible = false;
                    game_state.editing_prim_id = None;
                }
            });
        });

    if !dialog_open {
        push_revert(&mut game_state, &edit_dialog);
        edit_dialog.visible = false;
        edit_dialog.texture_picker_open = false;
        game_state.editing_prim_id = None;
    }

    // Texture picker window
    if edit_dialog.texture_picker_open && edit_dialog.visible {
        let mut picker_open = edit_dialog.texture_picker_open;
        let mut clicked_texture: Option<String> = None;
        Window::new("Texture Library")
            .open(&mut picker_open)
            .default_width(380.0)
            .show(egui.ctx_mut(), |ui| {
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    let cols = 4usize;
                    egui::Grid::new("tex_grid")
                        .num_columns(cols)
                        .spacing([8.0, 8.0])
                        .show(ui, |ui| {
                            for (i, entry) in texture_lib.entries.iter().enumerate() {
                                let selected =
                                    edit_dialog.texture_id.as_deref() == Some(&entry.id);
                                let cell_clicked = ui
                                    .vertical(|ui| {
                                        let clicked = if let Some(handle) =
                                            texture_lib.egui_handles.get(&entry.id)
                                        {
                                            let sized = egui::load::SizedTexture::new(
                                                handle.id(),
                                                egui::vec2(72.0, 72.0),
                                            );
                                            let resp = ui.add(
                                                egui::Image::new(sized)
                                                    .sense(egui::Sense::click()),
                                            );
                                            if selected {
                                                ui.painter().rect_stroke(
                                                    resp.rect,
                                                    2.0,
                                                    egui::Stroke::new(
                                                        2.5,
                                                        ui.visuals().selection.bg_fill,
                                                    ),
                                                    egui::StrokeKind::Outside,
                                                );
                                            }
                                            resp.clicked()
                                        } else {
                                            // Fallback text tile while egui handle is pending.
                                            ui.add_sized(
                                                [72.0, 72.0],
                                                egui::SelectableLabel::new(selected, &entry.name),
                                            )
                                            .clicked()
                                        };
                                        ui.label(
                                            egui::RichText::new(&entry.name)
                                                .small()
                                                .line_height(Some(14.0)),
                                        );
                                        clicked
                                    })
                                    .inner;
                                if cell_clicked {
                                    clicked_texture = Some(entry.id.clone());
                                }
                                if (i + 1) % cols == 0 {
                                    ui.end_row();
                                }
                            }
                            if !texture_lib.entries.is_empty()
                                && texture_lib.entries.len() % cols != 0
                            {
                                ui.end_row();
                            }
                        });
                });
            });
        if let Some(id) = clicked_texture {
            edit_dialog.texture_id = Some(id);
            picker_open = false;
        }
        edit_dialog.texture_picker_open = picker_open;
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
            original_texture_id: dialog.original_texture_id.clone(),
            ..Default::default()
        });
    }
}

/// Persist prim changes to the local database and update the Bevy world.
pub fn send_prim_mutations(
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
    db: Option<Res<Database>>,
    mut prim_query: Query<(Entity, &mut Prim, &mut Transform, Option<&MeshMaterial3d<StandardMaterial>>)>,
    region_query: Query<&Region>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    texture_cache: Res<PrimTextureCache>,
) {
    use crate::systems::prim_ops;

    if let Some(dialog_state) = game_state.pending_prim_save.take() {
        if dialog_state.is_new {
            let region_id = region_query.iter().next().map(|r| r.id).unwrap_or(1);
            let tex = dialog_state.texture_id.as_deref();

            let new_id: Option<i64> = db.as_ref().and_then(|db| {
                let conn = db.conn.lock().unwrap();
                prim_ops::db_create_prim(
                    &conn,
                    region_id,
                    &dialog_state.name,
                    &dialog_state.shape,
                    [
                        dialog_state.position[0] as f64,
                        dialog_state.position[1] as f64,
                        dialog_state.position[2] as f64,
                    ],
                    [
                        dialog_state.rotation[0] as f64,
                        dialog_state.rotation[1] as f64,
                        dialog_state.rotation[2] as f64,
                    ],
                    [
                        dialog_state.scale[0] as f64,
                        dialog_state.scale[1] as f64,
                        dialog_state.scale[2] as f64,
                    ],
                    [
                        dialog_state.color[0] as f64,
                        dialog_state.color[1] as f64,
                        dialog_state.color[2] as f64,
                    ],
                    tex,
                )
                .ok()
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
                        texture_id: dialog_state.texture_id.clone(),
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
            let tex = dialog_state.texture_id.as_deref();
            if let Some(ref db) = db {
                let conn = db.conn.lock().unwrap();
                let _ = prim_ops::db_update_prim(
                    &conn,
                    prim_id,
                    &dialog_state.name,
                    &dialog_state.shape,
                    [
                        dialog_state.position[0] as f64,
                        dialog_state.position[1] as f64,
                        dialog_state.position[2] as f64,
                    ],
                    [
                        dialog_state.rotation[0] as f64,
                        dialog_state.rotation[1] as f64,
                        dialog_state.rotation[2] as f64,
                    ],
                    [
                        dialog_state.scale[0] as f64,
                        dialog_state.scale[1] as f64,
                        dialog_state.scale[2] as f64,
                    ],
                    [
                        dialog_state.color[0] as f64,
                        dialog_state.color[1] as f64,
                        dialog_state.color[2] as f64,
                    ],
                    tex,
                );
            }
            for (entity, mut prim, mut transform, mat_handle_opt) in prim_query.iter_mut() {
                if prim.id == prim_id {
                    let texture_changed = prim.texture_id != dialog_state.texture_id;
                    prim.name = dialog_state.name.clone();
                    prim.shape = PrimShape::from_str(&dialog_state.shape);
                    prim.color = Color::srgb(
                        dialog_state.color[0],
                        dialog_state.color[1],
                        dialog_state.color[2],
                    );
                    prim.texture_id = dialog_state.texture_id.clone();
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
                    if texture_changed {
                        apply_texture_to_material(
                            mat_handle_opt,
                            &mut materials,
                            &texture_cache,
                            dialog_state.texture_id.as_deref(),
                        );
                    }
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
            for (entity, mut prim, mut transform, mat_handle_opt) in prim_query.iter_mut() {
                if prim.id == prim_id {
                    let texture_changed = prim.texture_id != revert.original_texture_id;
                    prim.name = revert.name.clone();
                    prim.shape = PrimShape::from_str(&revert.shape);
                    prim.color = Color::srgb(revert.color[0], revert.color[1], revert.color[2]);
                    prim.texture_id = revert.original_texture_id.clone();
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
                    if texture_changed {
                        apply_texture_to_material(
                            mat_handle_opt,
                            &mut materials,
                            &texture_cache,
                            revert.original_texture_id.as_deref(),
                        );
                    }
                    commands.entity(entity).insert(Selected);
                    game_state.selected_prim_id = Some(prim_id);
                    break;
                }
            }
        }
    }

    for prim_id in std::mem::take(&mut game_state.prims_to_delete) {
        if let Some(ref db) = db {
            let conn = db.conn.lock().unwrap();
            let _ = conn.execute("DELETE FROM prims WHERE id=?1", rusqlite::params![prim_id]);
        }
        for (entity, prim, _, _) in prim_query.iter() {
            if prim.id == prim_id {
                commands.entity(entity).despawn();
                tracing::info!(id = prim_id, "deleted prim");
                break;
            }
        }
    }
}

/// Applies (or clears) a texture on a material handle in-place, same frame.
fn apply_texture_to_material(
    mat_handle_opt: Option<&MeshMaterial3d<StandardMaterial>>,
    materials: &mut Assets<StandardMaterial>,
    cache: &PrimTextureCache,
    texture_id: Option<&str>,
) {
    let Some(mat_handle) = mat_handle_opt else {
        return;
    };
    let Some(mat) = materials.get_mut(&mat_handle.0) else {
        return;
    };
    mat.base_color_texture = texture_id
        .and_then(|id| cache.handles.get(id))
        .cloned()
        .map(|h| h.into());
}
