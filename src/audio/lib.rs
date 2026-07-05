use std::{
    num::NonZero,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
};

use dashmap::DashMap;
use parking_lot::Mutex;
use rodio::{MixerDeviceSink, Player, Source};
use strum::EnumTryAs;

use crate::audio::playback::{PlayerPreferences, SamplePlayer};

pub struct AudioPlayback {
    pub sink: MixerDeviceSink,
}

impl AudioPlayback {
    pub fn new() -> anyhow::Result<Self> {
        let sink = rodio::DeviceSinkBuilder::from_default_device()?
            .with_buffer_size(rodio::cpal::BufferSize::Fixed(1024))
            .with_channels(NonZero::new(2).unwrap())
            .open_sink_or_fallback()?;

        Ok(Self { sink })
    }
}

#[derive(Debug, Clone)]
pub enum AudioThreadMessage {
    CreatePlayer(u64),
    LoadPlayer {
        id: u64,
        fs_src: PathBuf,
    },
    UpdatePlayerPreferences {
        id: u64,
        preferences: PlayerPreferences,
    },
}

#[derive(EnumTryAs, Clone, Debug)]
pub enum AudioThreadReply {
    CreatedPlayer(u64),
    UpdatedPlayer(Result<(), String>),
}

pub struct AudioThreadHandler {
    thread_input: Sender<AudioThreadMessage>,
    thread_output: Mutex<Receiver<AudioThreadReply>>,

    pub sample_players: Arc<DashMap<u64, SamplePlayer>>,
}

impl AudioThreadHandler {
    /// Sends a message to the thread and waits for its reply.
    /// If a reply never comes this block indefinitely.
    pub fn create_exchange(&self, message: AudioThreadMessage) -> anyhow::Result<AudioThreadReply> {
        // Send message to thread
        self.thread_input.send(message)?;

        // Wait for reply from thread
        Ok(self.thread_output.lock().recv()?)
    }

    /// Sends a message but does not wait for the thread's reply.
    /// This is most useful for sending commands which dont have a reply.
    pub fn send_command(&self, message: AudioThreadMessage) -> anyhow::Result<()> {
        // Send reply to thread
        self.thread_input.send(message)?;

        Ok(())
    }
}

/// It is very important that this thread should never try to block or panic since that would stop audio playback completely.
/// We should implement a method to restart this thread if it crashes.
pub fn create_playback_thread() -> anyhow::Result<AudioThreadHandler> {
    // Create an audio playback unit
    let audio_playback = AudioPlayback::new()?;

    // Create channel pair for communication with this audio thread
    // This is where we send the information in
    let (application_input_handle, input_receiver) =
        std::sync::mpsc::channel::<AudioThreadMessage>();

    // This is where we receive information from the thread
    let (thread_sender, application_output_handle) = std::sync::mpsc::channel::<AudioThreadReply>();

    // Create thread handler so that the main thread (or any other) can communicate with this thread directly.
    let currently_available_players = Arc::new(DashMap::new());

    let thread_handler = AudioThreadHandler {
        thread_output: Mutex::new(application_output_handle),
        thread_input: application_input_handle,
        sample_players: currently_available_players.clone(),
    };

    // Create the playback thread
    std::thread::spawn(move || {
        let audio_playback = audio_playback;
        let currently_available_players = currently_available_players.clone();

        // This thread should never quit
        loop {
            // Receive the message from the channel
            match input_receiver.recv() {
                Ok(msg) => {
                    // Handle the certain type of message from the enum
                    match msg {
                        AudioThreadMessage::CreatePlayer(id) => {
                            // Create a player and return it to the main thread.
                            let player = Player::connect_new(audio_playback.sink.mixer());

                            // Store the player in the available players' list
                            currently_available_players.insert(
                                id,
                                SamplePlayer {
                                    player: Arc::new(player),
                                    total_duration: None,
                                    preferences: PlayerPreferences::default(),
                                },
                            );

                            // The main thread should never quit so we unwrap here. (Channels are kept alive in the Application's state)
                            thread_sender
                                .send(AudioThreadReply::CreatedPlayer(id))
                                .unwrap();
                        }
                        AudioThreadMessage::LoadPlayer { id, fs_src } => {
                            // Try getting the player from its id.
                            let load_result =
                                if let Some(mut query) = currently_available_players.get_mut(&id) {
                                    let sample_player = query.value_mut();

                                    // Try fetching the source from fs
                                    let source = std::fs::File::open(fs_src)
                                        .map_err(|err| err.to_string())
                                        .and_then(|file| {
                                            rodio::Decoder::try_from(file)
                                                .map_err(|err| err.to_string())
                                        });

                                    // Try to decode the source itself
                                    match source {
                                        Ok(src) => {
                                            // Set the total duration field of the sample
                                            sample_player.total_duration = src.total_duration();

                                            // Append the source to the player and return Ok
                                            sample_player.player.append(src);

                                            Ok(())
                                        }
                                        Err(err) => Err(err.to_string()),
                                    }
                                } else {
                                    Err(format!("Player not available on Id: `{id}`"))
                                };

                            thread_sender
                                .send(AudioThreadReply::UpdatedPlayer(load_result))
                                .unwrap();
                        }
                        AudioThreadMessage::UpdatePlayerPreferences { id, preferences } => {
                            // Try getting the player from its id.
                            let load_result =
                                if let Some(query) = currently_available_players.get(&id) {
                                    let sample_player = query.value();

                                    // Set the properties of the player
                                    sample_player.player.set_volume(preferences.volume);
                                    sample_player.player.set_speed(preferences.speed);

                                    Ok(())
                                } else {
                                    Err(format!("Player not available on Id: `{id}`"))
                                };

                            thread_sender
                                .send(AudioThreadReply::UpdatedPlayer(load_result))
                                .unwrap();
                        }
                    }
                }
                Err(_err) => {
                    eprintln!("Error in Audio Thread: {_err}");
                }
            }
        }
    });

    // Return thread handles
    Ok(thread_handler)
}
