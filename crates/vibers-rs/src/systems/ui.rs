//! Prim UI system (Phase 5 - ADR-017).
//!
//! Renders context menus and edit dialogs using egui.

use super::egui_manager::EguiManager;
use crate::components::Prim;
use crate::resources::{ContextMenuState, EditDialogState, GameState};
use bevy::prelude::*;
use egui::Window;

/// Render context menu when a prim is right-clicked
pub fn render_context_menu(
    mut egui: ResMut<EguiManager>,
    mut context_menu: ResMut<ContextMenuState>,
    mut edit_dialog: ResMut<EditDialogState>,
    mut game_state: ResMut<GameState>,
    prim_query: Query<(&Prim, &Transform)>,
) {
    if !context_menu.visible {
        return;
    }

    // Debug: Context menu is visible
    if let Some(prim_id) = context_menu.hit_prim_id {
        tracing::info!(prim_id, "context menu: edit/delete prim");
    } else {
        tracing::info!("context menu: create prim");
    }

    println!("\n\x1b[92m=== CONTEXT MENU OPEN ===");
    if let Some(prim_id) = context_menu.hit_prim_id {
        println!("Prim ID: {}", prim_id);
        println!("Press [E] to Edit, [D] to Delete, or click buttons\x1b[0m");
    } else {
        println!("Empty space");
        println!("Press [C] to Create Prim, or click button\x1b[0m");
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
                    // Load the prim properties into the edit dialog
                    for (prim, transform) in prim_query.iter() {
                        if prim.id == prim_id {
                            game_state.editing_prim_id = Some(prim_id);
                            edit_dialog.prim_id = Some(prim_id);
                            edit_dialog.is_new = false;
                            edit_dialog.name = prim.name.clone();
                            edit_dialog.color = {
                                let c = prim.color.to_linear();
                                [c.red, c.green, c.blue]
                            };
                            edit_dialog.shape = format!("{:?}", prim.shape).to_lowercase();
                            let pos = transform.translation;
                            edit_dialog.position = [pos.x, pos.y, pos.z];
                            let rot = transform.rotation.to_euler(EulerRot::XYZ);
                            edit_dialog.rotation = [rot.0, rot.1, rot.2];
                            let scale = transform.scale;
                            edit_dialog.scale = [scale.x, scale.y, scale.z];
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
                    edit_dialog.position = [hit.x, hit.y, hit.z];
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
        println!("\x1b[91m[Context menu closed]\x1b[0m");
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

    // Debug: Edit dialog is visible
    let dialog_type = if edit_dialog.is_new { "CREATE" } else { "EDIT" };
    tracing::info!(dialog_type, name = %edit_dialog.name, "edit dialog open");

    let title = if edit_dialog.is_new { "CREATE" } else { "EDIT" };
    println!("\x1b[94m=== {} PRIM DIALOG ===", title);
    println!("Name: {}", edit_dialog.name);
    println!(
        "Position: [{:.1}, {:.1}, {:.1}]",
        edit_dialog.position[0], edit_dialog.position[1], edit_dialog.position[2]
    );
    println!("Press [S] to Save, [ESC] to Cancel\x1b[0m");

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

            // Position fields
            ui.label("Position:");
            ui.horizontal(|ui| {
                ui.label("X:");
                ui.add(egui::DragValue::new(&mut edit_dialog.position[0]).speed(0.1));
                ui.label("Y:");
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
        edit_dialog.visible = false;
        game_state.editing_prim_id = None;
        println!("\x1b[91m[Edit dialog closed]\x1b[0m");
    }
}

/// Render UI status text overlay for debugging
pub fn render_ui_overlay(context_menu: Res<ContextMenuState>, edit_dialog: Res<EditDialogState>) {
    if context_menu.visible {
        if let Some(prim_id) = context_menu.hit_prim_id {
            println!("\n>>> CONTEXT MENU VISIBLE - Prim ID: {}", prim_id);
            println!("    [E] Edit  [D] Delete  [ESC] Cancel");
        } else {
            println!("\n>>> CONTEXT MENU VISIBLE - Empty Space");
            println!("    [C] Create Prim  [ESC] Cancel");
        }
    }

    if edit_dialog.visible {
        let title = if edit_dialog.is_new { "CREATE" } else { "EDIT" };
        println!("\n>>> {} DIALOG VISIBLE", title);
        println!("    Name: {}", edit_dialog.name);
        println!(
            "    Pos: [{:.1}, {:.1}, {:.1}]",
            edit_dialog.position[0], edit_dialog.position[1], edit_dialog.position[2]
        );
        println!("    [S] Save  [ESC] Cancel");
    }
}

/// Send prim messages to the network (handles Save from edit dialog)
pub fn send_prim_mutations(mut game_state: ResMut<GameState>, _prim_query: Query<&Prim>) {
    // Handle prim save
    if let Some(dialog_state) = game_state.pending_prim_save.take() {
        if dialog_state.is_new {
            println!("\x1b[92m[SAVING NEW PRIM]\x1b[0m");
            println!("  Name: {}", dialog_state.name);
            println!("  Shape: {}", dialog_state.shape);
            println!(
                "  Position: [{:.1}, {:.1}, {:.1}]",
                dialog_state.position[0], dialog_state.position[1], dialog_state.position[2]
            );
            // TODO: Send CreatePrim message to server
        } else if let Some(prim_id) = dialog_state.prim_id {
            println!("\x1b[92m[UPDATING PRIM {}]\x1b[0m", prim_id);
            println!("  Name: {}", dialog_state.name);
            // TODO: Send UpdatePrim message to server
        }
    }

    // Handle prim delete
    if let Some(prim_id) = game_state.prim_to_delete.take() {
        println!("\x1b[91m[DELETING PRIM {}]\x1b[0m", prim_id);
        tracing::info!(prim_id, "deleting prim");
        // TODO: Send DeletePrim message to server
    }
}
