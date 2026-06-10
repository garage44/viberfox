use super::egui_manager::EguiManager;
use super::prim_ops;
use crate::components::{Prim, PrimShape, Region};
use crate::resources::{AiAssistantState, AiConfig, Database, DisplayMessage, PendingAiResponse};
use bevy::prelude::*;
use std::sync::{Arc, Mutex};

const SYSTEM_PROMPT: &str = "\
You are an AI assistant embedded in Viberfox, a 3D world editor. \
You can create, update, and delete 3D primitives (prims) using tools.\n\
\n\
Coordinate system: Y is up. To rest an object on the ground plane, set position.y to half its \
scale.y (e.g. a box with scale.y=4 should have position.y=2). \
Colors are in [0.0, 1.0] range per channel (r, g, b). \
Available shapes: box, sphere, cylinder, cone, torus.\n\
\n\
For city buildings: tall thin boxes (scale ~[3,8,3]) for towers, wide low boxes for podiums, \
cylinders for pillars. Vary heights, widths, and colors for visual interest. \
Spread buildings along the X or Z axis with ~5-unit spacing.";

fn tools_schema() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "list_prims",
            "description": "List all prims currently in the scene with their properties.",
            "input_schema": {"type": "object", "properties": {}}
        },
        {
            "name": "create_prim",
            "description": "Create a new 3D primitive in the scene.",
            "input_schema": {
                "type": "object",
                "required": ["name", "shape", "position", "scale", "color"],
                "properties": {
                    "name": {"type": "string"},
                    "shape": {"type": "string", "enum": ["box", "sphere", "cylinder", "cone", "torus"]},
                    "position": {
                        "type": "object",
                        "required": ["x", "y", "z"],
                        "properties": {
                            "x": {"type": "number"},
                            "y": {"type": "number", "description": "Vertical (up) axis. Use scale.y/2 to rest on ground."},
                            "z": {"type": "number"}
                        }
                    },
                    "scale": {
                        "type": "object",
                        "required": ["x", "y", "z"],
                        "properties": {
                            "x": {"type": "number"},
                            "y": {"type": "number"},
                            "z": {"type": "number"}
                        }
                    },
                    "color": {
                        "type": "object",
                        "required": ["r", "g", "b"],
                        "properties": {
                            "r": {"type": "number", "description": "[0.0-1.0]"},
                            "g": {"type": "number", "description": "[0.0-1.0]"},
                            "b": {"type": "number", "description": "[0.0-1.0]"}
                        }
                    },
                    "texture_id": {
                        "type": "string",
                        "description": "Optional texture id (filename stem, e.g. 'brick'). Omit for solid color."
                    }
                }
            }
        },
        {
            "name": "update_prim",
            "description": "Update an existing prim's properties. Only include the fields you want to change.",
            "input_schema": {
                "type": "object",
                "required": ["prim_id"],
                "properties": {
                    "prim_id": {"type": "integer"},
                    "name": {"type": "string"},
                    "shape": {"type": "string", "enum": ["box", "sphere", "cylinder", "cone", "torus"]},
                    "position": {
                        "type": "object",
                        "properties": {
                            "x": {"type": "number"},
                            "y": {"type": "number"},
                            "z": {"type": "number"}
                        }
                    },
                    "scale": {
                        "type": "object",
                        "properties": {
                            "x": {"type": "number"},
                            "y": {"type": "number"},
                            "z": {"type": "number"}
                        }
                    },
                    "color": {
                        "type": "object",
                        "properties": {
                            "r": {"type": "number"},
                            "g": {"type": "number"},
                            "b": {"type": "number"}
                        }
                    },
                    "texture_id": {
                        "type": "string",
                        "description": "Optional texture id (filename stem, e.g. 'brick'). Set to null to clear."
                    }
                }
            }
        },
        {
            "name": "delete_prim",
            "description": "Delete a prim from the scene.",
            "input_schema": {
                "type": "object",
                "required": ["prim_id"],
                "properties": {
                    "prim_id": {"type": "integer"}
                }
            }
        }
    ])
}

fn fire_request(
    api_key: String,
    model: String,
    messages: Vec<serde_json::Value>,
) -> PendingAiResponse {
    let cell: PendingAiResponse = Arc::new(Mutex::new(None));
    let cell_clone = cell.clone();
    std::thread::spawn(move || {
        let result = call_anthropic(&api_key, &model, &messages);
        *cell_clone.lock().unwrap() = Some(result);
    });
    cell
}

fn call_anthropic(
    api_key: &str,
    model: &str,
    messages: &[serde_json::Value],
) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "system": SYSTEM_PROMPT,
        "tools": tools_schema(),
        "messages": messages,
    });
    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let req = ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", api_key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json");

    let resp_str = match req.send_string(&body_str) {
        Ok(resp) => resp.into_string().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp
                .into_string()
                .unwrap_or_else(|_| "unknown HTTP error".to_string());
            return Err(format!("HTTP {code}: {body}"));
        }
        Err(e) => return Err(e.to_string()),
    };

    serde_json::from_str(&resp_str).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// UI system
// ---------------------------------------------------------------------------

pub fn render_ai_panel(
    mut egui: ResMut<EguiManager>,
    mut state: ResMut<AiAssistantState>,
    config: Res<AiConfig>,
) {
    let ctx = egui.ctx_mut();

    if !state.open {
        let screen = ctx.screen_rect();
        egui::Window::new("ai_tab")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(screen.max.x - 42.0, screen.max.y * 0.5 - 16.0))
            .fixed_size([38.0, 32.0])
            .show(ctx, |ui| {
                if ui.button("AI").clicked() {
                    state.open = true;
                }
            });
        return;
    }

    egui::SidePanel::right("ai_panel")
        .resizable(true)
        .default_width(320.0)
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.heading("AI Assistant");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        state.open = false;
                    }
                });
            });
            ui.separator();

            if config.api_key.is_none() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Set ANTHROPIC_API_KEY in .env to enable.",
                );
                return;
            }

            let is_busy = state.pending.is_some();

            // Chat history
            let input_height = 80.0;
            let avail = ui.available_height() - input_height - 16.0;
            egui::ScrollArea::vertical()
                .max_height(avail.max(40.0))
                .auto_shrink([false; 2])
                .stick_to_bottom(state.scroll_to_bottom)
                .show(ui, |ui| {
                    for msg in &state.display_messages {
                        if msg.is_user {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Min),
                                |ui| {
                                    ui.group(|ui| {
                                        ui.set_max_width(ui.available_width() * 0.85);
                                        ui.label(&msg.text);
                                    });
                                },
                            );
                        } else {
                            ui.group(|ui| {
                                ui.set_max_width(ui.available_width());
                                ui.label(&msg.text);
                            });
                        }
                        ui.add_space(4.0);
                    }
                    if is_busy {
                        ui.spinner();
                        ui.label("Thinking…");
                    }
                });

            state.scroll_to_bottom = false;

            if let Some(err) = &state.error.clone() {
                ui.colored_label(egui::Color32::RED, err);
            }

            ui.separator();

            // Input row
            ui.add_enabled_ui(!is_busy, |ui| {
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut state.input)
                        .desired_rows(3)
                        .hint_text("Ask the AI to create or edit prims… (Ctrl+Enter)"),
                );

                let submit = ui.button("Send").clicked()
                    || (resp.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl));

                if submit && !state.input.trim().is_empty() {
                    state.error = None;
                    state.pending_send = true;
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Response polling + tool execution
// ---------------------------------------------------------------------------

pub fn poll_ai_response(
    mut state: ResMut<AiAssistantState>,
    config: Res<AiConfig>,
    mut commands: Commands,
    db: Option<Res<Database>>,
    prim_query: Query<(Entity, &Prim, &Transform)>,
    region_query: Query<&Region>,
) {
    // Fire request when the user submitted a message
    if state.pending_send && state.pending.is_none() {
        state.pending_send = false;
        let text = state.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        state.input.clear();
        state
            .display_messages
            .push(DisplayMessage { is_user: true, text: text.clone() });
        state
            .api_messages
            .push(serde_json::json!({"role": "user", "content": text}));
        state.scroll_to_bottom = true;
        state.tool_round = 0;

        if let Some(key) = config.api_key.clone() {
            state.pending = Some(fire_request(key, config.model.clone(), state.api_messages.clone()));
        }
        return;
    }

    // Poll for response
    let response = {
        let Some(cell) = &state.pending else {
            return;
        };
        let mut guard = cell.lock().unwrap();
        guard.take()
    };
    let Some(response) = response else {
        return;
    };
    state.pending = None;

    match response {
        Err(e) => {
            tracing::error!("Anthropic API error: {}", e);
            state.error = Some(format!("Error: {e}"));
        }
        Ok(json) => {
            // Surface API-level errors (e.g. bad auth key)
            if json.get("error").is_some() {
                let msg = json["error"]["message"]
                    .as_str()
                    .unwrap_or("unknown API error")
                    .to_string();
                state.error = Some(format!("API error: {msg}"));
                return;
            }

            // Store assistant message in history (role + content only)
            state.api_messages.push(serde_json::json!({
                "role": "assistant",
                "content": json["content"],
            }));

            let Some(content) = json["content"].as_array() else {
                return;
            };

            let mut text_parts: Vec<String> = Vec::new();
            let mut tool_calls: Vec<serde_json::Value> = Vec::new();

            for block in content {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(t) = block["text"].as_str() {
                            text_parts.push(t.to_string());
                        }
                    }
                    Some("tool_use") => {
                        tool_calls.push(block.clone());
                    }
                    _ => {}
                }
            }

            if !text_parts.is_empty() {
                state.display_messages.push(DisplayMessage {
                    is_user: false,
                    text: text_parts.join("\n"),
                });
                state.scroll_to_bottom = true;
            }

            if tool_calls.is_empty() {
                return; // conversation turn complete
            }

            if state.tool_round >= 8 {
                state.error = Some("Reached tool-call limit (8 rounds).".to_string());
                return;
            }
            state.tool_round += 1;

            // Execute tools and collect results
            let mut tool_results: Vec<serde_json::Value> = Vec::new();
            for call in &tool_calls {
                let tool_id = call["id"].as_str().unwrap_or("").to_string();
                let tool_name = call["name"].as_str().unwrap_or("").to_string();
                let input = &call["input"];

                let result_text = run_tool(
                    &tool_name,
                    input,
                    &mut commands,
                    &db,
                    &prim_query,
                    &region_query,
                );

                state.display_messages.push(DisplayMessage {
                    is_user: false,
                    text: format!("[ {} → {} ]", tool_name, result_text),
                });
                state.scroll_to_bottom = true;

                tool_results.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": tool_id,
                    "content": result_text,
                }));
            }

            // Feed tool results back and fire next request
            state
                .api_messages
                .push(serde_json::json!({"role": "user", "content": tool_results}));

            if let Some(key) = config.api_key.clone() {
                state.pending =
                    Some(fire_request(key, config.model.clone(), state.api_messages.clone()));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tool execution
// ---------------------------------------------------------------------------

fn run_tool(
    name: &str,
    input: &serde_json::Value,
    commands: &mut Commands,
    db: &Option<Res<Database>>,
    prim_query: &Query<(Entity, &Prim, &Transform)>,
    region_query: &Query<&Region>,
) -> String {
    match name {
        "list_prims" => {
            let list: Vec<serde_json::Value> = prim_query
                .iter()
                .map(|(_, p, t)| {
                    let lin = p.color.to_linear();
                    serde_json::json!({
                        "id": p.id,
                        "name": p.name,
                        "shape": format!("{:?}", p.shape).to_lowercase(),
                        "position": {"x": t.translation.x, "y": t.translation.y, "z": t.translation.z},
                        "scale":    {"x": t.scale.x,       "y": t.scale.y,       "z": t.scale.z},
                        "color":    {"r": lin.red,          "g": lin.green,       "b": lin.blue},
                    })
                })
                .collect();
            serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
        }

        "create_prim" => {
            let prim_name = input["name"].as_str().unwrap_or("Prim").to_string();
            let shape_str = input["shape"].as_str().unwrap_or("box").to_string();
            let pos = vec3_from(input, "position");
            let scale = vec3_from(input, "scale");
            let color = color_from(input);
            let texture_id = input["texture_id"].as_str().map(|s| s.to_string());
            let region_id = region_query.iter().next().map(|r| r.id).unwrap_or(1);

            let new_id = db.as_ref().and_then(|db| {
                let conn = db.conn.lock().unwrap();
                prim_ops::db_create_prim(
                    &conn,
                    region_id,
                    &prim_name,
                    &shape_str,
                    [pos.x as f64, pos.y as f64, pos.z as f64],
                    [0.0, 0.0, 0.0],
                    [scale.x as f64, scale.y as f64, scale.z as f64],
                    [color[0] as f64, color[1] as f64, color[2] as f64],
                    texture_id.as_deref(),
                )
                .ok()
            });

            match new_id {
                Some(id) => {
                    commands.spawn((
                        Prim {
                            id,
                            region_id,
                            name: prim_name.clone(),
                            shape: PrimShape::from_str(&shape_str),
                            color: Color::srgb(color[0], color[1], color[2]),
                            texture_id: texture_id.clone(),
                        },
                        Transform::from_xyz(pos.x, pos.y, pos.z)
                            .with_scale(Vec3::new(scale.x, scale.y, scale.z)),
                    ));
                    format!("Created '{}' id={}", prim_name, id)
                }
                None => "Failed: no database".to_string(),
            }
        }

        "update_prim" => {
            let prim_id = match input["prim_id"].as_i64() {
                Some(id) => id,
                None => return "Error: missing prim_id".to_string(),
            };

            // Find current values to merge with partial updates
            let existing = prim_query.iter().find(|(_, p, _)| p.id == prim_id);
            let (cur_name, cur_shape, cur_pos, cur_scale, cur_color) = match existing {
                Some((_, p, t)) => {
                    let lin = p.color.to_linear();
                    (
                        p.name.clone(),
                        format!("{:?}", p.shape).to_lowercase(),
                        t.translation,
                        t.scale,
                        [lin.red, lin.green, lin.blue],
                    )
                }
                None => return format!("Error: prim {} not found", prim_id),
            };

            let new_name = input["name"].as_str().unwrap_or(&cur_name).to_string();
            let new_shape = input["shape"].as_str().unwrap_or(&cur_shape).to_string();
            let new_pos = if input.get("position").is_some() {
                vec3_from(input, "position")
            } else {
                cur_pos
            };
            let new_scale = if input.get("scale").is_some() {
                vec3_from(input, "scale")
            } else {
                cur_scale
            };
            let new_color = if input.get("color").is_some() {
                color_from(input)
            } else {
                cur_color
            };
            let new_texture_id = if input.get("texture_id").is_some() {
                input["texture_id"].as_str().map(|s| s.to_string())
            } else {
                prim_query
                    .iter()
                    .find(|(_, p, _)| p.id == prim_id)
                    .and_then(|(_, p, _)| p.texture_id.clone())
            };

            // Persist to DB
            if let Some(db) = db {
                let conn = db.conn.lock().unwrap();
                let _ = prim_ops::db_update_prim(
                    &conn,
                    prim_id,
                    &new_name,
                    &new_shape,
                    [new_pos.x as f64, new_pos.y as f64, new_pos.z as f64],
                    [0.0, 0.0, 0.0],
                    [new_scale.x as f64, new_scale.y as f64, new_scale.z as f64],
                    [new_color[0] as f64, new_color[1] as f64, new_color[2] as f64],
                    new_texture_id.as_deref(),
                );
            }

            // Update the Bevy entity
            if let Some((entity, existing_prim, _)) =
                prim_query.iter().find(|(_, p, _)| p.id == prim_id)
            {
                let region_id = existing_prim.region_id;
                commands.entity(entity).insert((
                    Prim {
                        id: prim_id,
                        region_id,
                        name: new_name.clone(),
                        shape: PrimShape::from_str(&new_shape),
                        color: Color::srgb(new_color[0], new_color[1], new_color[2]),
                        texture_id: new_texture_id,
                    },
                    Transform::from_xyz(new_pos.x, new_pos.y, new_pos.z)
                        .with_scale(Vec3::new(new_scale.x, new_scale.y, new_scale.z)),
                ));
            }

            format!("Updated prim {}", prim_id)
        }

        "delete_prim" => {
            let prim_id = match input["prim_id"].as_i64() {
                Some(id) => id,
                None => return "Error: missing prim_id".to_string(),
            };

            if let Some(db) = db {
                let conn = db.conn.lock().unwrap();
                let _ = prim_ops::db_delete_prim(&conn, prim_id);
            }

            for (entity, prim, _) in prim_query.iter() {
                if prim.id == prim_id {
                    commands.entity(entity).despawn();
                    break;
                }
            }

            format!("Deleted prim {}", prim_id)
        }

        other => format!("Unknown tool: {}", other),
    }
}

// ---------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------

fn vec3_from(v: &serde_json::Value, key: &str) -> Vec3 {
    let obj = &v[key];
    Vec3::new(
        obj["x"].as_f64().unwrap_or(0.0) as f32,
        obj["y"].as_f64().unwrap_or(0.0) as f32,
        obj["z"].as_f64().unwrap_or(0.0) as f32,
    )
}

fn color_from(v: &serde_json::Value) -> [f32; 3] {
    let obj = &v["color"];
    [
        obj["r"].as_f64().unwrap_or(0.5) as f32,
        obj["g"].as_f64().unwrap_or(0.5) as f32,
        obj["b"].as_f64().unwrap_or(0.5) as f32,
    ]
}
