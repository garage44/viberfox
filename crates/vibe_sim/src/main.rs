//! Headless simulation server (ADR-007, ADR-008, ADR-010–014).

mod cli;
mod config;
mod db;
mod net;
mod state;

use clap::Parser;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, RwLock};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let sim_cli = cli::SimCli::parse();
    let mut config = config::SimConfig::load()?;
    config.apply_cli(&sim_cli);
    let config = Arc::new(config);
    tracing::info!(
        listen = %config.listen,
        db = %config.database_path,
        tick_hz = config.tick_hz,
        aoi = config.aoi_radius,
        tile_template = %config.osm_tile_url_template,
        "vibe_sim"
    );

    let conn = db::open_and_migrate(&config.database_path)?;
    let (regions, prims) = db::load_world(&conn)?;
    let conn = Arc::new(std::sync::Mutex::new(conn));

    let world = Arc::new(RwLock::new(state::SimWorld::new(
        regions,
        prims,
        config.aoi_radius,
        conn.clone(),
    )));

    let (tx_snap, _) = broadcast::channel::<Vec<u8>>(256);
    let world_tick = world.clone();
    let config_tick = config.clone();
    tokio::spawn(net::tick_loop(world_tick, config_tick, tx_snap.clone()));

    let listener = TcpListener::bind(&config.listen).await?;
    tracing::info!("listening on {}", config.listen);

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::info!(%addr, "accepted");
        let world_c = world.clone();
        let cfg_c = config.clone();
        let rx = tx_snap.subscribe();
        let tx_c = tx_snap.clone();
        let conn_c = conn.clone();
        tokio::spawn(async move {
            if let Err(e) = net::handle_connection(stream, world_c, cfg_c, rx, tx_c, conn_c).await {
                tracing::warn!(%addr, "client ended: {e:#}");
            }
        });
    }
}
