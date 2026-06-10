use bevy::prelude::*;
use std::sync::Mutex;
use crate::components::{Region, Prim, PrimShape};
use crate::db::schema::{RegionRow, PrimRow};
use crate::resources::{Database, GameState};

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
        let mut stmt = match conn.prepare("SELECT * FROM prims ORDER BY created_at DESC") {
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
                created_at: row.get(16)?,
                updated_at: row.get(17)?,
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
