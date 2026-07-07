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

pub struct HostAudioPlayback {
    pub sink: MixerDeviceSink,
}

impl HostAudioPlayback {
    pub fn new() -> anyhow::Result<Self> {
        let sink = rodio::DeviceSinkBuilder::from_default_device()?
            .with_buffer_size(rodio::cpal::BufferSize::Fixed(1024))
            .with_channels(NonZero::new(2).unwrap())
            .open_sink_or_fallback()?;

        Ok(Self { sink })
    }
}

/// Message types for communicating to the thread.
#[derive(Debug, Clone)]
pub enum AudioThreadMessage {
    /// Tells the thread to create a player with a specific id.
    CreatePlayer(u64),
    /// Tells the thread to load a source to the player with the specific id.
    LoadPlayer { id: u64, fs_src: PathBuf },
    /// Tell the thread to update the settings of a specific player.
    UpdatePlayerPreferences {
        id: u64,
        preferences: PlayerPreferences,
    },
}

/// Messages the audio thread can reply with.
#[derive(EnumTryAs, Clone, Debug)]
pub enum AudioThreadReply {
    /// Notifies the sender if a player has been created.
    CreatedPlayer(u64),
    /// Notifies the sender if a player has been updated.
    UpdatedPlayer(Result<(), String>),
}

/// The audio thread handler struct is always bound to one thread - the one it is created with.
/// It contains fields useful for communicating with the thread.
/// Please note that this thread handler is only suitable for communication between two threads.
pub struct AudioThreadHandler {
    /// A channel to send messages to the thread.
    thread_input: Sender<AudioThreadMessage>,
    /// A channel to receive replies from the thread.
    thread_output: Mutex<Receiver<AudioThreadReply>>,

    /// A list of players bound to a specific id.
    /// Normally this is not cleaned up, since every player contains its own settings (ie volume) - and they dont hold much space anyway.
    pub sample_players: Arc<DashMap<u64, SamplePlayer>>,
}

impl AudioThreadHandler {
    pub fn create_empty() -> Self {
        let (thread_input, _) = std::sync::mpsc::channel::<AudioThreadMessage>();
        let (_, thread_output) = std::sync::mpsc::channel::<AudioThreadReply>();

        Self {
            thread_input,
            thread_output: Mutex::new(thread_output),
            sample_players: Arc::new(DashMap::new()),
        }
    }

    /// Sends a message to the thread and waits for its reply.
    /// If a reply never comes this blocks indefinitely.
    pub fn create_exchange(&self, message: AudioThreadMessage) -> anyhow::Result<AudioThreadReply> {
        // Send message to thread
        self.thread_input.send(message)?;

        // Wait for reply from thread
        Ok(self.thread_output.lock().recv()?)
    }
}

/// It is very important that this thread should never try to block or panic since that would stop audio playback completely.
/// We should implement a method to restart this thread if it crashes.
pub fn create_playback_thread(
    audio_playback: Arc<HostAudioPlayback>,
) -> anyhow::Result<AudioThreadHandler> {
    // Create channel pair for communication with this audio thread
    // This is where we send the information in
    let (application_input_handle, input_receiver) =
        std::sync::mpsc::channel::<AudioThreadMessage>();

    // This is where we receive information from the thread
    let (thread_sender, application_output_handle) = std::sync::mpsc::channel::<AudioThreadReply>();

    // Create thread handler so that the main thread (or any other) can communicate with this thread directly.
    let currently_available_players = Arc::new(DashMap::new());

    // Create thread handler instance
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
                                            // Clear out the player before inserting the new source.
                                            sample_player.player.stop();

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
                    eprintln!("Error in Audio Playback Thread: {_err}");
                }
            }
        }
    });

    // Return thread handles
    Ok(thread_handler)
}
