use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::sync::Mutex;
use crate::components::{Region, Prim, PrimShape};
use crate::db::schema::{RegionRow, PrimRow};
use crate::resources::{Database, GameState, PrimTextureCache, TextureLibrary};
use crate::systems::egui_manager::EguiManager;
use vibe_core::TextureEntry;

pub fn init_database(mut commands: Commands) {
    let db_path = "data/regions.db";

    // Ensure data directory exists (SQLite does not create parent dirs)
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("❌ Failed to create data directory: {}", e);
            return;
        }
    }

    match crate::db::schema::init_database(db_path) {
        Ok(conn) => {
            println!("✅ Database initialized");
            commands.insert_resource(Database { conn: Mutex::new(conn) });
        }
        Err(e) => {
            eprintln!("❌ Failed to initialize database: {}", e);
        }
    }
}

pub fn load_regions(
    db: Res<Database>,
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
) {
    if game_state.regions_loaded {
        return;
    }

    let regions_result: Result<Vec<RegionRow>, rusqlite::Error> = {
        let conn = db.conn.lock().unwrap();
        let mut stmt = match conn.prepare("SELECT * FROM regions ORDER BY created_at DESC") {
            Ok(stmt) => stmt,
            Err(e) => {
                eprintln!("Error preparing regions query: {}", e);
                return;
            }
        };

        let rows_result = stmt.query_map([], |row| {
            Ok(RegionRow {
                id: row.get(0)?,
                name: row.get(1)?,
                latitude: row.get(2)?,
                longitude: row.get(3)?,
                tile_x: row.get(4)?,
                tile_y: row.get(5)?,
                tile_z: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        });

        match rows_result {
            Ok(rows) => rows.collect(),
            Err(e) => {
                eprintln!("Error querying regions: {}", e);
                return;
            }
        }
    };

    match regions_result {
        Ok(regions) => {
            let count = regions.len();
            for region in regions {
                // Create region entity (rendering will be handled separately)
                // Only add Region component, no Transform or other components
                commands.spawn(Region {
                    id: region.id,
                    name: region.name.clone(),
                    latitude: region.latitude,
                    longitude: region.longitude,
                    tile_x: region.tile_x,
                    tile_y: region.tile_y,
                    tile_z: region.tile_z,
                    sim_origin: None,
                });
                tracing::debug!("spawned region entity for '{}'", region.name);
            }
            tracing::info!("loaded {} regions", count);
            game_state.regions_loaded = true;
        }
        Err(e) => {
            eprintln!("Error querying regions: {}", e);
        }
    }
}

pub fn load_prims(
    db: Res<Database>,
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
) {
    if game_state.prims_loaded {
        return;
    }

    let prims_result: Result<Vec<PrimRow>, rusqlite::Error> = {
        let conn = db.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT id, region_id, name, shape, \
             position_x, position_y, position_z, \
             rotation_x, rotation_y, rotation_z, \
             scale_x, scale_y, scale_z, \
             color_r, color_g, color_b, \
             texture_id, created_at, updated_at \
             FROM prims ORDER BY created_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                eprintln!("Error preparing prims query: {}", e);
                return;
            }
        };

        let rows_result = stmt.query_map([], |row| {
            Ok(PrimRow {
                id: row.get(0)?,
                region_id: row.get(1)?,
                name: row.get(2)?,
                shape: row.get(3)?,
                position_x: row.get(4)?,
                position_y: row.get(5)?,
                position_z: row.get(6)?,
                rotation_x: row.get(7)?,
                rotation_y: row.get(8)?,
                rotation_z: row.get(9)?,
                scale_x: row.get(10)?,
                scale_y: row.get(11)?,
                scale_z: row.get(12)?,
                color_r: row.get(13)?,
                color_g: row.get(14)?,
                color_b: row.get(15)?,
                texture_id: row.get(16)?,
                created_at: row.get(17)?,
                updated_at: row.get(18)?,
            })
        });

        match rows_result {
            Ok(rows) => rows.collect(),
            Err(e) => {
                eprintln!("Error querying prims: {}", e);
                return;
            }
        }
    };

    match prims_result {
        Ok(prims) => {
            let count = prims.len();
            for prim in prims {
                // Create prim entity (rendering will be handled separately)
                commands.spawn((
                    Prim {
                        id: prim.id,
                        region_id: prim.region_id,
                        name: prim.name.clone(),
                        shape: PrimShape::from_str(&prim.shape),
                        color: Color::srgb(prim.color_r, prim.color_g, prim.color_b),
                        texture_id: prim.texture_id.clone(),
                    },
                    Transform::from_xyz(prim.position_x, prim.position_y, prim.position_z)
                        .with_rotation(Quat::from_euler(
                            EulerRot::XYZ,
                            prim.rotation_x,
                            prim.rotation_y,
                            prim.rotation_z,
                        ))
                        .with_scale(Vec3::new(prim.scale_x, prim.scale_y, prim.scale_z)),
                ));
            }
            tracing::info!("loaded {} prims", count);
            game_state.prims_loaded = true;
        }
        Err(e) => {
            eprintln!("Error querying prims: {}", e);
        }
    }
}

/// Offline mode: scan `data/textures/*.{png,jpg,jpeg}` and pre-load them.
pub fn load_local_textures(
    egui_manager: Res<EguiManager>,
    mut texture_lib: ResMut<TextureLibrary>,
    mut texture_cache: ResMut<PrimTextureCache>,
    mut images: ResMut<Assets<Image>>,
) {
    let dir = "data/textures";
    let path = std::path::Path::new(dir);
    if !path.exists() {
        return;
    }

    let mut entries: Vec<TextureEntry> = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(path) {
        for entry in read_dir.flatten() {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "png" | "jpg" | "jpeg") {
                continue;
            }
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                let id = stem.to_string();
                if texture_cache.handles.contains_key(&id) {
                    continue; // already loaded (e.g. both .png and .jpg with same stem)
                }
                let name = title_case(stem);
                if let Ok(bytes) = std::fs::read(&p) {
                    if let Some(rgba) = decode_to_rgba(&bytes) {
                        let (w, h) = rgba.dimensions();
                        let raw = rgba.into_raw();

                        // Egui ColorImage for the picker thumbnail.
                        let pixels: Vec<egui::Color32> = raw.chunks_exact(4)
                            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                            .collect();
                        let color_image = egui::ColorImage { size: [w as usize, h as usize], pixels };
                        let egui_handle = egui_manager.ctx.load_texture(
                            &id,
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );
                        texture_lib.egui_handles.insert(id.clone(), egui_handle);

                        // Bevy Image for material texturing.
                        let bevy_img = Image::new(
                            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                            TextureDimension::D2,
                            raw,
                            TextureFormat::Rgba8UnormSrgb,
                            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                        );
                        let handle = images.add(bevy_img);
                        texture_cache.handles.insert(id.clone(), handle);
                        entries.push(TextureEntry { id, name });
                    }
                }
            }
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let count = entries.len();
    texture_lib.entries = entries;
    if count > 0 {
        tracing::info!(count, "loaded local textures from {dir}");
    }
}

fn decode_to_rgba(bytes: &[u8]) -> Option<image::RgbaImage> {
    Some(image::load_from_memory(bytes).ok()?.to_rgba8())
}

fn title_case(s: &str) -> String {
    let spaced = s.replace('_', " ").replace('-', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
