use std::{sync::Arc, time::Duration};

use rodio::Player;

#[derive(Debug, Clone, Copy)]
pub struct PlayerPreferences {
    pub speed: f32,
    pub volume: f32,
}

impl Default for PlayerPreferences {
    fn default() -> Self {
        Self {
            speed: 1.0,
            volume: 1.0,
        }
    }
}

#[derive(Clone)]
pub struct SamplePlayer {
    pub player: Arc<Player>,
    pub total_duration: Option<Duration>,
    pub preferences: PlayerPreferences,
}
