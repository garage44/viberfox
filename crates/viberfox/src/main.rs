use bevy::asset::AssetPlugin;
use bevy::log::LogPlugin;
use bevy::pbr::light_consts::lux::AMBIENT_DAYLIGHT;
use bevy::prelude::*;
use bevy_atmosphere::prelude::*;
use clap::Parser;
use std::path::PathBuf;

mod components;
mod db;
mod resources;
mod systems;
mod utils;

use components::Avatar;
use resources::{
    AiAssistantState, AiConfig, AvatarState, CameraState, ConnectAddr, ContextMenuState, Database,
    EditDialogState, GameState, LocalAvatarSimId, MarqueeState, MouseState, OsmTileUrlTemplate,
    PrimTextureCache, TextureLibrary,
};
use systems::egui_manager::EguiPlugin;
use systems::*;

#[derive(Parser, Debug)]
#[command(name = "viberfox")]
struct Cli {
    /// Connect to a `vibe_sim` instance (TCP, postcard messages).
    #[arg(long)]
    connect: Option<String>,
}

fn main() {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off,viberfox=info")),
        )
        .init();

    let mut app = App::new();
    let asset_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .display()
        .to_string();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<LogPlugin>()
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Viberfox".into(),
                    resolution: (1280.0, 720.0).into(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: asset_dir,
                ..default()
            }),
    )
    .add_plugins(EguiPlugin)
    .add_plugins(AtmospherePlugin)
    .insert_resource(AtmosphereModel::default())
    .init_resource::<GameState>()
    .init_resource::<AvatarState>()
    .init_resource::<LocalAvatarSimId>()
    .init_resource::<CameraState>()
    .init_resource::<MouseState>()
    .init_resource::<systems::tile_loader::TileCache>()
    .init_resource::<OsmTileUrlTemplate>()
    .init_resource::<ContextMenuState>()
    .init_resource::<EditDialogState>()
    .init_resource::<systems::gizmo::GizmoState>()
    .init_resource::<MarqueeState>()
    .init_resource::<TextureLibrary>()
    .init_resource::<PrimTextureCache>()
    .insert_resource(AiConfig {
        api_key: std::env::var("ANTHROPIC_KEY").ok(),
        model: std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string()),
    })
    .init_resource::<AiAssistantState>();

    if let Some(addr) = cli.connect {
        app.insert_resource(ConnectAddr(addr));
    }

    app.add_systems(
        Startup,
        (
            database::init_database.run_if(no_connect_addr),
            database::load_local_textures.run_if(no_connect_addr),
            network::spawn_network_thread.run_if(has_connect_addr),
            systems::free_camera::setup_camera,
            spawn_avatar_entity,
            setup_sky,
        ),
    )
    .add_systems(
        Update,
        (
            network::apply_network_snapshot,
            network::create_egui_texture_handles
                .after(network::apply_network_snapshot),
            database::load_regions
                .run_if(has_database)
                .after(network::apply_network_snapshot),
            database::load_prims
                .run_if(has_database)
                .after(network::apply_network_snapshot),
            rendering::spawn_regions
                .after(database::load_regions)
                .after(network::apply_network_snapshot),
            rendering::spawn_prims
                .after(database::load_prims)
                .after(network::apply_network_snapshot),
            rendering::rebuild_prim_meshes
                .after(rendering::spawn_prims),
            rendering::refresh_prim_textures
                .after(rendering::spawn_prims)
                .after(network::apply_network_snapshot),
        ),
    )
    .add_systems(
        Update,
        (
            systems::tile_loader::load_region_tiles,
            rendering::update_region_materials,
        ),
    )
    .add_systems(
        Update,
        (
            systems::free_camera::camera_mode_toggle,
            avatar::handle_avatar_movement.after(network::apply_network_snapshot),
            avatar::smooth_online_avatar_display
                .after(network::apply_network_snapshot)
                .after(avatar::handle_avatar_movement),
            avatar::smooth_remote_avatars.after(avatar::smooth_online_avatar_display),
            avatar::tick_remote_avatar_motion_hint.after(avatar::smooth_remote_avatars),
            network::send_network_intent.after(avatar::handle_avatar_movement),
            network::send_observer_update
                .after(network::apply_network_snapshot)
                .after(avatar::handle_avatar_movement),
            systems::free_camera::camera_controls.after(avatar::smooth_online_avatar_display),
            avatar::spawn_avatar,
            avatar::update_fox_animation.after(avatar::handle_avatar_movement),
            avatar::update_remote_fox_animation.after(avatar::tick_remote_avatar_motion_hint),
            systems::debug::debug_region_entities.after(rendering::spawn_regions),
        ),
    )
    // Phase 4: Prim selection, raycasting, marquee, keyboard shortcuts
    .add_systems(
        Update,
        (
            systems::picking::prim_picking,
            systems::picking::update_marquee.after(systems::picking::prim_picking),
            systems::picking::render_marquee.after(systems::picking::update_marquee),
            systems::picking::handle_delete_key,
            systems::picking::highlight_selected_prim,
            systems::picking::unhighlight_deselected_prim,
        ),
    )
    // Phase 5: Context menu and edit dialog
    .add_systems(
        Update,
        (
            systems::ui::render_context_menu,
            systems::ui::render_edit_dialog,
            systems::ui::apply_live_prim_edits.after(systems::ui::render_edit_dialog),
            systems::ui::send_prim_mutations.after(systems::ui::render_edit_dialog),
        ),
    )
    // Phase 7: AI assistant panel
    .add_systems(
        Update,
        (
            systems::ai_assistant::render_ai_panel,
            systems::ai_assistant::poll_ai_response
                .after(systems::ai_assistant::render_ai_panel),
        ),
    )
    // Phase 6: Transform gizmos
    .add_systems(
        Update,
        (
            systems::gizmo::handle_gizmo_mode_input,
            systems::gizmo::render_gizmo_toolbar,
            systems::gizmo::render_gizmo_visuals,
            systems::gizmo::handle_gizmo_interaction,
        ),
    );

    app.run();
}

fn has_connect_addr(addr: Option<Res<ConnectAddr>>) -> bool {
    addr.is_some()
}

fn no_connect_addr(addr: Option<Res<ConnectAddr>>) -> bool {
    addr.is_none()
}

fn has_database(db: Option<Res<Database>>) -> bool {
    db.is_some()
}

fn spawn_avatar_entity(mut commands: Commands) {
    commands.spawn((
        Avatar,
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)).with_scale(Vec3::splat(0.02)),
    ));
}

fn setup_sky(mut commands: Commands, mut atmosphere: AtmosphereMut<Nishita>) {
    let sun_position = Vec3::new(0.3, 0.8, 0.5).normalize();
    atmosphere.sun_position = sun_position;

    commands.spawn((
        DirectionalLight {
            illuminance: AMBIENT_DAYLIGHT,
            ..default()
        },
        Transform::from_translation(Vec3::ZERO).looking_to(-sun_position, Vec3::Y),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.1,
        affects_lightmapped_meshes: true,
    });
}
