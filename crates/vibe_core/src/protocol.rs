//! Wire messages (ADR-009). TCP framing is length-delimited (ADR-008); each frame body is an **app frame**:
//! `protocol_version` (u16 LE) + `message_kind` (u16 LE) + `request_id` (u32 LE) + postcard(`NetMessage`).

use glam::Vec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ProtocolError;

/// Bump when the app-frame layout or postcard schema changes incompatibly.
pub const PROTOCOL_VERSION: u16 = 5;

const APP_HEADER_LEN: usize = 8;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    ClientHello = 1,
    ServerHelloAck = 2,
    ServerError = 3,
    ClientIntent = 4,
    ObserverUpdate = 5,
    WorldSnapshot = 6,
    PrimRemoved = 7,
    CreatePrim = 8,
    UpdatePrim = 9,
    DeletePrim = 10,
    PrimUpsert = 11,
}

impl MessageKind {
    #[must_use]
    pub fn from_wire(v: u16) -> Option<Self> {
        match v {
            1 => Some(Self::ClientHello),
            2 => Some(Self::ServerHelloAck),
            3 => Some(Self::ServerError),
            4 => Some(Self::ClientIntent),
            5 => Some(Self::ObserverUpdate),
            6 => Some(Self::WorldSnapshot),
            7 => Some(Self::PrimRemoved),
            8 => Some(Self::CreatePrim),
            9 => Some(Self::UpdatePrim),
            10 => Some(Self::DeletePrim),
            11 => Some(Self::PrimUpsert),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegionDto {
    pub id: i64,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub tile_x: i64,
    pub tile_y: i64,
    pub tile_z: i64,
    /// Sim-space origin for this region’s tile (matches `vibers-sim` grid layout).
    pub sim_x: f32,
    pub sim_y: f32,
    pub sim_z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrimDto {
    pub id: i64,
    pub region_id: i64,
    pub name: String,
    pub shape: String,
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
    pub color: [f32; 3],
    /// Optional texture key from standard library (ADR-017).
    #[serde(default)]
    pub texture_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AvatarStateDto {
    pub id: u64,
    pub position: Vec3,
    pub yaw: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetMessage {
    ClientHello {
        protocol_version: u16,
        client_token: String,
    },
    ServerHelloAck {
        session_id: Uuid,
        tick_hz: f32,
        your_avatar_id: u64,
        /// ADR-014: operator tile URL; `{z}/{x}/{y}` placeholders. Empty = client default.
        #[serde(default)]
        osm_tile_url_template: String,
    },
    ServerError {
        request_id: u32,
        code: u32,
        message: String,
    },
    ClientIntent {
        request_id: u32,
        move_x: f32,
        move_z: f32,
        /// World yaw replicated to others (fox / travel facing). Server applies this when moving instead of `atan2(velocity)`.
        display_yaw: f32,
        fly_up: bool,
        fly_down: bool,
    },
    ObserverUpdate {
        position: Vec3,
    },
    WorldSnapshot {
        tick: u64,
        regions: Vec<RegionDto>,
        prims: Vec<PrimDto>,
        avatars: Vec<AvatarStateDto>,
    },
    /// ADR-011 delta v0: remove prim by stable id (server-assigned).
    PrimRemoved {
        id: i64,
    },
    /// ADR-017: client requests creation of a new prim.
    CreatePrim {
        request_id: u32,
        region_id: i64,
        position: Vec3,
        shape: String,
    },
    /// ADR-017: client sends updated prim state (position, rotation, scale, color, texture, name).
    UpdatePrim {
        request_id: u32,
        prim_id: i64,
        position: Vec3,
        rotation: Vec3,
        scale: Vec3,
        color: [f32; 3],
        texture_id: Option<String>,
        name: String,
    },
    /// ADR-017: client requests deletion of a prim by id.
    DeletePrim {
        request_id: u32,
        prim_id: i64,
    },
    /// ADR-017: server broadcasts the full authoritative prim state after a create or update.
    PrimUpsert {
        prim: PrimDto,
    },
}

#[must_use]
pub fn message_kind(msg: &NetMessage) -> MessageKind {
    match msg {
        NetMessage::ClientHello { .. } => MessageKind::ClientHello,
        NetMessage::ServerHelloAck { .. } => MessageKind::ServerHelloAck,
        NetMessage::ServerError { .. } => MessageKind::ServerError,
        NetMessage::ClientIntent { .. } => MessageKind::ClientIntent,
        NetMessage::ObserverUpdate { .. } => MessageKind::ObserverUpdate,
        NetMessage::WorldSnapshot { .. } => MessageKind::WorldSnapshot,
        NetMessage::PrimRemoved { .. } => MessageKind::PrimRemoved,
        NetMessage::CreatePrim { .. } => MessageKind::CreatePrim,
        NetMessage::UpdatePrim { .. } => MessageKind::UpdatePrim,
        NetMessage::DeletePrim { .. } => MessageKind::DeletePrim,
        NetMessage::PrimUpsert { .. } => MessageKind::PrimUpsert,
    }
}

#[must_use]
pub fn message_request_id(msg: &NetMessage) -> u32 {
    match msg {
        NetMessage::ClientIntent { request_id, .. } => *request_id,
        NetMessage::ServerError { request_id, .. } => *request_id,
        NetMessage::CreatePrim { request_id, .. } => *request_id,
        NetMessage::UpdatePrim { request_id, .. } => *request_id,
        NetMessage::DeletePrim { request_id, .. } => *request_id,
        _ => 0,
    }
}

/// Raw postcard body (no app header). Prefer [`encode_app_frame`] / [`decode_app_frame`] on the wire.
#[must_use]
pub fn encode_message(msg: &NetMessage) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(msg)
}

#[must_use]
pub fn decode_message(bytes: &[u8]) -> Result<NetMessage, postcard::Error> {
    postcard::from_bytes(bytes)
}

/// Full ADR-009 app frame: version + kind + request_id + postcard payload.
#[must_use]
pub fn encode_app_frame(msg: &NetMessage) -> Result<Vec<u8>, postcard::Error> {
    let body = encode_message(msg)?;
    let kind = message_kind(msg) as u16;
    let rid = message_request_id(msg);
    let mut out = Vec::with_capacity(APP_HEADER_LEN + body.len());
    out.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&rid.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

#[must_use]
pub fn decode_app_frame(bytes: &[u8]) -> Result<NetMessage, ProtocolError> {
    if bytes.len() < APP_HEADER_LEN {
        return Err(ProtocolError::FrameTooShort(bytes.len()));
    }
    let ver = u16::from_le_bytes(bytes[0..2].try_into().unwrap());
    if ver != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(ver));
    }
    let kind_wire = u16::from_le_bytes(bytes[2..4].try_into().unwrap());
    let Some(kind) = MessageKind::from_wire(kind_wire) else {
        return Err(ProtocolError::UnknownMessageKind(kind_wire));
    };
    let _request_id = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let msg = decode_message(&bytes[APP_HEADER_LEN..])?;
    if message_kind(&msg) != kind {
        return Err(ProtocolError::KindMismatch {
            header: kind_wire,
            body: message_kind(&msg) as u16,
        });
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_hello_app_frame() {
        let m = NetMessage::ClientHello {
            protocol_version: PROTOCOL_VERSION,
            client_token: "test".into(),
        };
        let b = encode_app_frame(&m).unwrap();
        let m2 = decode_app_frame(&b).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn roundtrip_snapshot_app_frame() {
        let m = NetMessage::WorldSnapshot {
            tick: 42,
            regions: vec![],
            prims: vec![],
            avatars: vec![],
        };
        let b = encode_app_frame(&m).unwrap();
        let m2 = decode_app_frame(&b).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn roundtrip_create_prim_app_frame() {
        let m = NetMessage::CreatePrim {
            request_id: 123,
            region_id: 1,
            position: Vec3::new(10.0, 5.0, 20.0),
            shape: "box".into(),
        };
        let b = encode_app_frame(&m).unwrap();
        let m2 = decode_app_frame(&b).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn roundtrip_update_prim_app_frame() {
        let m = NetMessage::UpdatePrim {
            request_id: 124,
            prim_id: 42,
            position: Vec3::new(10.0, 5.0, 20.0),
            rotation: Vec3::new(0.0, 90.0, 0.0),
            scale: Vec3::new(2.0, 2.0, 2.0),
            color: [1.0, 0.0, 0.0],
            texture_id: Some("brick".into()),
            name: "My Prim".into(),
        };
        let b = encode_app_frame(&m).unwrap();
        let m2 = decode_app_frame(&b).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn roundtrip_delete_prim_app_frame() {
        let m = NetMessage::DeletePrim {
            request_id: 125,
            prim_id: 42,
        };
        let b = encode_app_frame(&m).unwrap();
        let m2 = decode_app_frame(&b).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn roundtrip_prim_upsert_app_frame() {
        let prim = PrimDto {
            id: 42,
            region_id: 1,
            name: "Updated Prim".into(),
            shape: "sphere".into(),
            position: Vec3::new(15.0, 8.0, 25.0),
            rotation: Vec3::new(45.0, 45.0, 0.0),
            scale: Vec3::new(1.5, 1.5, 1.5),
            color: [0.0, 1.0, 0.0],
            texture_id: Some("grass".into()),
        };
        let m = NetMessage::PrimUpsert { prim: prim.clone() };
        let b = encode_app_frame(&m).unwrap();
        let m2 = decode_app_frame(&b).unwrap();
        assert_eq!(m, m2);
    }
}
