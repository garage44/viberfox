use bevy::prelude::*;
use rusqlite::Connection;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::mpsc::UnboundedSender;
use vibe_core::NetMessage;

// ---------------------------------------------------------------------------
// AI assistant
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct AiConfig {
    pub api_key: Option<String>,
    pub model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: "claude-haiku-4-5-20251001".to_string(),
        }
    }
}

pub struct DisplayMessage {
    pub is_user: bool,
    pub text: String,
}

pub type PendingAiResponse = Arc<Mutex<Option<Result<serde_json::Value, String>>>>;

#[derive(Resource, Default)]
pub struct AiAssistantState {
    pub open: bool,
    pub input: String,
    pub display_messages: Vec<DisplayMessage>,
    /// Full conversation history in Anthropic Messages API format.
    pub api_messages: Vec<serde_json::Value>,
    /// Shared cell written by background thread, polled each frame.
    pub pending: Option<PendingAiResponse>,
    /// Set by render_ai_panel when user submits; consumed by poll_ai_response.
    pub pending_send: bool,
    /// Number of tool-call rounds in the current turn (safety cap).
    pub tool_round: u8,
    pub error: Option<String>,
    pub scroll_to_bottom: bool,
}

#[derive(Resource)]
pub struct Database {
    pub conn: Mutex<Connection>,
}

#[derive(Resource, Default)]
pub struct GameState {
    pub selected_prim_id: Option<i64>,
    pub regions_loaded: bool,
    pub prims_loaded: bool,
    pub editing_prim_id: Option<i64>,
    pub pending_prim_save: Option<EditDialogState>,
    pub pending_prim_revert: Option<EditDialogState>,
    /// Queued prim deletions (processed by send_prim_mutations each frame).
    pub prims_to_delete: Vec<i64>,
}

/// State for rubber-band (marquee) multi-select.
#[derive(Resource, Default)]
pub struct MarqueeState {
    /// Screen position where the mouse was pressed before we know if it's a click or drag.
    pub press_pos: Option<Vec2>,
    /// True while the rubber-band rectangle is being drawn.
    pub active: bool,
    /// Fixed corner (where the drag began).
    pub start: Vec2,
    /// Trailing corner (tracks the cursor).
    pub end: Vec2,
    /// Shift was held at drag start → add to existing selection instead of replacing.
    pub shift_add: bool,
}

/// Context menu state for prim interaction (right-click menus)
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct ContextMenuState {
    pub visible: bool,
    pub screen_pos: Vec2,
    pub hit_point: Vec3,
    pub hit_prim_id: Option<i64>,
    pub hit_region_id: Option<i64>,
}

/// Edit dialog state for prim properties
#[derive(Resource, Default, Debug, Clone)]
pub struct EditDialogState {
    pub visible: bool,
    pub is_new: bool,
    pub prim_id: Option<i64>,
    pub name: String,
    pub shape: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub color: [f32; 3],
    pub texture_id: Option<String>,
    // Snapshot taken when the dialog opens; used to build a revert payload on Cancel.
    pub original_name: String,
    pub original_shape: String,
    pub original_position: [f32; 3],
    pub original_rotation: [f32; 3],
    pub original_scale: [f32; 3],
    pub original_color: [f32; 3],
}

#[derive(Resource)]
pub struct AvatarState {
    /// Authoritative sim position (from server when online).
    pub position: Vec3,
    /// Visual follow target: smoothed toward `position` online so camera and mesh do not stutter at tick rate.
    pub display_position: Vec3,
    /// Offline / local yaw for tank controls (radians).
    pub rotation: f32,
    /// Online: replicated world yaw from snapshots (authoritative for reconciliation).
    pub sim_facing_yaw: f32,
    /// Online: `fox_facing_yaw − π` for legacy parity; movement uses camera azimuth, not this field.
    pub online_tank_yaw: f32,
    pub is_flying: bool,
    pub is_walking: bool,
}

impl Default for AvatarState {
    fn default() -> Self {
        let p = Vec3::new(0.0, 2.2, 0.0);
        Self {
            position: p,
            display_position: p,
            rotation: 0.0,
            sim_facing_yaw: 0.0,
            online_tank_yaw: 0.0,
            is_flying: false,
            is_walking: false,
        }
    }
}

#[derive(Resource)]
pub struct CameraState {
    pub mode: CameraMode,
    pub distance: f32,
    pub azimuth: f32,
    pub pitch: f32,
    pub pan_offset: Option<Vec2>, // Last mouse position for delta calculation
    pub free_camera_rotation: Vec2, // pitch, yaw
}

#[derive(Resource, Default)]
#[allow(dead_code)] // drag/pan state kept for future mouse interaction
pub struct MouseState {
    pub last_position: Option<Vec2>,
    pub is_dragging: bool,
    pub is_panning: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CameraMode {
    Avatar, // Default: camera follows avatar
    Free,   // Free camera mode (FPS-style)
}

impl Default for CameraMode {
    fn default() -> Self {
        CameraMode::Avatar
    }
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            mode: CameraMode::Avatar,
            distance: 5.0,
            azimuth: 0.0,
            pitch: std::f32::consts::PI / 6.0,
            pan_offset: None,
            free_camera_rotation: Vec2::new(0.0, 0.0),
        }
    }
}

/// OSM tile URL template (`{z}`/`{x}`/`{y}`); filled from server handshake when online (ADR-014).
#[derive(Resource, Clone)]
pub struct OsmTileUrlTemplate(pub Arc<Mutex<String>>);

impl Default for OsmTileUrlTemplate {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(String::new())))
    }
}

/// When set, client connects to `vibe_sim` instead of loading local SQLite world.
#[derive(Resource, Clone)]
pub struct ConnectAddr(pub String);

#[derive(Resource)]
pub struct OnlineSession {
    pub intent_tx: UnboundedSender<NetMessage>,
}

/// After the first `WorldSnapshot`, static regions/prims are not torn down each sim tick (ADR-011).
#[derive(Resource, Default)]
pub struct NetworkSyncState {
    pub received_initial_world: bool,
}

/// Set from `ServerHelloAck.your_avatar_id` so we can pick the local row in `WorldSnapshot::avatars`.
#[derive(Resource, Default, Clone, Copy)]
pub struct LocalAvatarSimId(pub Option<u64>);

/// Incoming messages from the network thread (`Receiver` is not `Sync`; wrap in `Mutex`).
#[derive(Resource)]
pub struct NetworkMailbox {
    pub rx: Mutex<std::sync::mpsc::Receiver<NetMessage>>,
}

impl NetworkMailbox {
    pub fn lock_rx(&self) -> MutexGuard<'_, std::sync::mpsc::Receiver<NetMessage>> {
        self.rx.lock().expect("network mailbox mutex poisoned")
    }
}
