use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PositionEvent {
    pub x: f32,
    pub y: f32,
}
