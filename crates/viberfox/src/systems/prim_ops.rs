use anyhow::Result;
use rusqlite::Connection;

/// Twist / taper / top-shear / slice in storage units
/// (degrees, −1..1, fraction). Bundled to keep the prim DB signatures readable.
#[derive(Clone, Copy)]
pub struct WarpParams {
    pub twist_begin: f64,
    pub twist_end: f64,
    pub taper_x: f64,
    pub taper_y: f64,
    pub top_shear_x: f64,
    pub top_shear_y: f64,
    pub slice_begin: f64,
    pub slice_end: f64,
}

impl Default for WarpParams {
    fn default() -> Self {
        Self {
            twist_begin: 0.0,
            twist_end: 0.0,
            taper_x: 0.0,
            taper_y: 0.0,
            top_shear_x: 0.0,
            top_shear_y: 0.0,
            slice_begin: 0.0,
            slice_end: 1.0,
        }
    }
}

pub fn db_create_prim(
    conn: &Connection,
    region_id: i64,
    name: &str,
    shape: &str,
    pos: [f64; 3],
    rot: [f64; 3],
    scale: [f64; 3],
    color: [f64; 3],
    texture_id: Option<&str>,
    path_cut_begin: f64,
    path_cut_end: f64,
    hollow: f64,
    warp: WarpParams,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO prims \
         (region_id, name, shape, \
          position_x, position_y, position_z, \
          rotation_x, rotation_y, rotation_z, \
          scale_x, scale_y, scale_z, \
          color_r, color_g, color_b, texture_id, \
          path_cut_begin, path_cut_end, hollow, \
          twist_begin, twist_end, taper_x, taper_y, \
          top_shear_x, top_shear_y, slice_begin, slice_end, \
          created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,\
                 ?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,\
                 datetime('now'),datetime('now'))",
        rusqlite::params![
            region_id,
            name,
            shape,
            pos[0],
            pos[1],
            pos[2],
            rot[0],
            rot[1],
            rot[2],
            scale[0],
            scale[1],
            scale[2],
            color[0],
            color[1],
            color[2],
            texture_id,
            path_cut_begin,
            path_cut_end,
            hollow,
            warp.twist_begin,
            warp.twist_end,
            warp.taper_x,
            warp.taper_y,
            warp.top_shear_x,
            warp.top_shear_y,
            warp.slice_begin,
            warp.slice_end,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn db_update_prim(
    conn: &Connection,
    prim_id: i64,
    name: &str,
    shape: &str,
    pos: [f64; 3],
    rot: [f64; 3],
    scale: [f64; 3],
    color: [f64; 3],
    texture_id: Option<&str>,
    path_cut_begin: f64,
    path_cut_end: f64,
    hollow: f64,
    warp: WarpParams,
) -> Result<()> {
    conn.execute(
        "UPDATE prims SET name=?1, shape=?2, \
         position_x=?3, position_y=?4, position_z=?5, \
         rotation_x=?6, rotation_y=?7, rotation_z=?8, \
         scale_x=?9, scale_y=?10, scale_z=?11, \
         color_r=?12, color_g=?13, color_b=?14, texture_id=?15, \
         path_cut_begin=?16, path_cut_end=?17, hollow=?18, \
         twist_begin=?19, twist_end=?20, taper_x=?21, taper_y=?22, \
         top_shear_x=?23, top_shear_y=?24, slice_begin=?25, slice_end=?26, \
         updated_at=datetime('now') WHERE id=?27",
        rusqlite::params![
            name,
            shape,
            pos[0],
            pos[1],
            pos[2],
            rot[0],
            rot[1],
            rot[2],
            scale[0],
            scale[1],
            scale[2],
            color[0],
            color[1],
            color[2],
            texture_id,
            path_cut_begin,
            path_cut_end,
            hollow,
            warp.twist_begin,
            warp.twist_end,
            warp.taper_x,
            warp.taper_y,
            warp.top_shear_x,
            warp.top_shear_y,
            warp.slice_begin,
            warp.slice_end,
            prim_id,
        ],
    )?;
    Ok(())
}

pub fn db_delete_prim(conn: &Connection, prim_id: i64) -> Result<()> {
    conn.execute("DELETE FROM prims WHERE id=?1", rusqlite::params![prim_id])?;
    Ok(())
}
