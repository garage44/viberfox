use anyhow::Result;
use rusqlite::Connection;

pub fn db_create_prim(
    conn: &Connection,
    region_id: i64,
    name: &str,
    shape: &str,
    pos: [f64; 3],
    rot: [f64; 3],
    scale: [f64; 3],
    color: [f64; 3],
) -> Result<i64> {
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
) -> Result<()> {
    conn.execute(
        "UPDATE prims SET name=?1, shape=?2, \
         position_x=?3, position_y=?4, position_z=?5, \
         rotation_x=?6, rotation_y=?7, rotation_z=?8, \
         scale_x=?9, scale_y=?10, scale_z=?11, \
         color_r=?12, color_g=?13, color_b=?14, \
         updated_at=datetime('now') WHERE id=?15",
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
            prim_id,
        ],
    )?;
    Ok(())
}

pub fn db_delete_prim(conn: &Connection, prim_id: i64) -> Result<()> {
    conn.execute("DELETE FROM prims WHERE id=?1", rusqlite::params![prim_id])?;
    Ok(())
}
