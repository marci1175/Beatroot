use std::{
    collections::HashMap,
    fmt::Debug,
    hash::{Hash, Hasher},
    ops::Add,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

// static GLOBAL_SAMPLE_ID: GlobalID = GlobalID::new(0);

use crate::{
    audio::{host::HOST_STATE, ingest::calculate_beat_pos, playback::MasterPlaybackThread},
    internals::{
        sample::{SampleProperties, generate_sample_waveform},
        utils::find_value_inbetween,
    },
    plugins::{InstanceResult, PluginDescriptor},
    ui::{
        fx_map::{Node, NodeMap, NodeType},
        panels::{
            lib::{
                GlobalState, Panel, PanelStates, display_error_as_toast, random_color_with_opacity,
            },
            media::WorkspaceSampleAttributes,
        },
    },
};
use chrono::NaiveTime;
use egui::{
    Align2, Color32, FontId, Id, Layout, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, Ui,
    UiBuilder, Vec2, Widget, vec2,
};
use egui_toast::{Toast, ToastStyle};
use indexmap::IndexMap;
use parking_lot::{Mutex, RwLock};

const TRACK_HEIGHT: f32 = 100.0;
const MINIMUM_TRACK_HEIGHT: f32 = 10.;

// Set the height of the tracks (the horizontal space between two lines in the "grid")
const BEAT_WIDTH: usize = 25;

// Colors
const BAR_TRACK_SEPARATOR: Color32 = Color32::GRAY;
const STROKE_WIDTH: f32 = 1.0f32;

// This indicates that the track label is 4 bars wide
const TRACK_LABEL_WIDTH: usize = BEAT_WIDTH * 4;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TrackCustomization {
    pub label_text: String,
    pub label_text_color: Color32,
    pub label_color: Color32,
    pub height: f32,

    /// This just makes it so that if the track's height has ever been set this will be true and it wont be automatically deleted
    pub height_set: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SampleInstance {
    pub id: usize,
    pub name: String,
    pub color: Color32,
    pub path: PathBuf,
    pub properties: SampleProperties,
    pub waveform_map: Option<Vec<[f32; 2]>>,
}

impl TrackCustomization {
    fn named_default(nth: usize, label_color: Color32, text_color: Color32) -> Self {
        Self {
            label_text: format!("Track {nth}"),
            label_text_color: text_color,
            label_color,
            height: TRACK_HEIGHT,

            height_set: false,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, Hash, Eq, PartialEq)]
pub struct Position {
    pub track: usize,
    pub beat: usize,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum PlaybackState {
    /// When the plaback is currently ongoing
    Playing,
    /// When the placback has been stopped
    Paused,
    /// When the player hasnt been initalized
    #[default]
    Stopped,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct PlaylistPreferences {
    pub waveform_color: Color32,
    pub default_track_label_color: Color32,
    pub default_track_label_text_color: Color32,
    pub cursor_color: Color32,
}

impl Default for PlaylistPreferences {
    fn default() -> Self {
        Self {
            waveform_color: Color32::WHITE,
            default_track_label_color: Color32::ORANGE,
            default_track_label_text_color: Color32::WHITE,
            cursor_color: Color32::GREEN,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistState {
    /// Can be modified with the bpm slider.
    pub bpm: Arc<Mutex<f64>>,

    /// This indicates how much the user has scrolled.
    pub grid_offset: Vec2,

    /// Track customization
    pub custom_tracks: HashMap<usize, TrackCustomization>,

    /// All samples are contained in this map.
    /// All samples have to line up to one beat (position) (I may change this later) and to one track.
    /// Multiple samples may be present at the same location.
    /// The index of each entry in this map is unique and can be used to manage effects applied on the samples themselves.
    /// The reason this is an Arc<RwLock> is because the ingest thread runs in a different thread and it would only make sense to be able to hold a separate reference just to the playlist's samples.
    pub samples: Arc<RwLock<IndexMap<Position, Vec<SampleInstance>>>>,

    pub playback_state: PlaybackState,

    pub playlist_preferences: PlaylistPreferences,

    pub time_signature_numerator: Arc<Mutex<i32>>,
    pub time_signature_denominator: Arc<Mutex<i32>>,
}

impl PlaylistState {
    pub fn get_sample_count(&self) -> usize {
        self.samples
            .read()
            .iter()
            .map(|(_pos, samples)| samples.len())
            .sum()
    }
}

impl Default for PlaylistState {
    fn default() -> Self {
        Self {
            bpm: Arc::new(Mutex::new(120.)),
            grid_offset: Default::default(),
            custom_tracks: Default::default(),
            samples: Default::default(),
            playback_state: Default::default(),
            playlist_preferences: PlaylistPreferences::default(),
            time_signature_numerator: Arc::new(Mutex::new(4)),
            time_signature_denominator: Arc::new(Mutex::new(4)),
        }
    }
}

const BPM_PRESETS: &[f64] = &[
    60.0, 70.0, 80.0, 90.0, 100.00, 110.0, 120.0, 128.0, 140.0, 165.0, 174.0,
];

pub fn playlist_ui(
    this: &Panel,
    ui: &mut Ui,
    (panels_state, master_playback): (Arc<PanelStates>, Arc<MasterPlaybackThread>),
    global_state: GlobalState,
) {
    let state = &panels_state.playlist_panel;

    // Get the default track color
    let preferences = state.read().playlist_preferences;

    // Redraw ui always to make the cursor smooth
    // For some reason i cant just redraw when its playing
    ui.ctx().request_repaint();

    // Draw the main options / tools for this ui
    ui.horizontal(|ui| {
        let current_playback_state = state.read().playback_state.clone();

        // Display playback main controls based on current state
        match current_playback_state {
            PlaybackState::Playing => {
                if ui.button("Pause").clicked() {
                    state.write().playback_state = PlaybackState::Paused;
                    global_state.master_playback.playback_stopper.stop();
                };
            }
            PlaybackState::Paused => {
                if ui.button("Unpause").clicked() {
                    state.write().playback_state = PlaybackState::Playing;
                    global_state.master_playback.playback_stopper.go();
                };
            }
            PlaybackState::Stopped => {
                if ui.button("Play").clicked() {
                    state.write().playback_state = PlaybackState::Paused;
                    global_state.master_playback.playback_stopper.go();
                }
            }
        }

        // Only enable this button if its not stopped
        ui.add_enabled_ui(current_playback_state != PlaybackState::Stopped, |ui| {
            if ui.button("Stop").clicked() {
                state.write().playback_state = PlaybackState::Stopped;

                // Reset player's ingest offset
            }
        });

        ui.separator();

        if ui.button("Patterns").clicked() {};

        ui.label("bpm");

        let write = state.write();
        let playlist_bpm = &mut *write.bpm.lock();
        ui.add(egui::Slider::new(playlist_bpm, 10.0..=522.0).fixed_decimals(3))
            .context_menu(|ui| {
                ui.label("Presets");

                ui.separator();

                for bpm in BPM_PRESETS {
                    if ui.button(format!("{bpm} bpm")).clicked() {
                        *playlist_bpm = *bpm;
                    }
                }
            });
    });

    ui.separator();

    // Paint the background black, and draw on top of that
    let playlist_rect = ui.available_rect_before_wrap();

    ui.painter_at(playlist_rect)
        .rect_filled(ui.available_rect_before_wrap(), 0., Color32::BLACK);

    // The total grid's offset (the amount the user has scrolled.)
    let grid_offset = state.read().grid_offset;

    let y_offset_ratio = grid_offset.y / playlist_rect.height();
    let x_offset_ratio = grid_offset.x / playlist_rect.width();

    // Track the positions of the lines drawn so that we can visualize the preview of a sample in the playlist.
    // `first_visible_beat` tells us which absolute beat number `beat_lines[0]` corresponds to,
    // since the vec itself is scroll-relative (index 0 = "first beat currently on screen").
    let (first_visible_beat, beat_lines) =
        beat_outlines(ui, playlist_rect, x_offset_ratio, BEAT_WIDTH as f32);

    // Initalize the track lines list with the topmost line first.
    let mut track_lines = vec![[
        Pos2::new(playlist_rect.left(), playlist_rect.top()),
        Pos2::new(playlist_rect.right(), playlist_rect.top()),
    ]];

    let mut current_height = playlist_rect.top() + y_offset_ratio;

    // The index for tracking the painting of each track
    let mut idx = 0;

    let max_height = playlist_rect.bottom() - y_offset_ratio;

    // Indexes of the tracks which fulfill a certain criterium
    let mut first_visible_track_idx = 0;
    let mut last_visible_track_idx = 0;
    let mut is_first_track_visible = false;

    // Draw track labels (filled rect) and track separator lines
    // This rectangle takes up four bar widths
    while current_height < max_height {
        let y_coord = current_height;

        // Try getting the customization state for the current label
        let label_customization = get_track_customization(state, idx, &preferences);

        let top = (y_coord + y_offset_ratio).max(playlist_rect.top());
        let bottom =
            (y_coord + y_offset_ratio + label_customization.height).min(playlist_rect.bottom());

        let is_visible = !(top >= playlist_rect.bottom() || bottom <= playlist_rect.top());

        // This will always be set to the last visible track's idx after this while loop
        if !is_first_track_visible {
            first_visible_track_idx = idx;
        }

        // Only display the track if its acutally visible
        // Render currently visible tracks
        if is_visible {
            is_first_track_visible = true;

            // Get access to the track customizations
            let custom_tracks: &mut HashMap<usize, TrackCustomization> =
                &mut state.write().custom_tracks;

            // Draw track labels
            track_label(
                ui,
                playlist_rect,
                idx,
                &label_customization,
                top,
                bottom,
                custom_tracks,
                &preferences,
            );

            // Draw separator lines
            let separator_line = track_separator(
                ui,
                playlist_rect,
                y_offset_ratio,
                idx,
                y_coord,
                &label_customization,
                custom_tracks,
                &preferences,
            );

            // This will automatically set the index to the last visible track's index
            last_visible_track_idx = idx;

            track_lines.push(separator_line);
        }

        // Add the consumed height to the current height
        current_height += label_customization.height;

        // Track indexes too
        idx += 1;
    }

    // The space after the track labels
    let usable_playlist_rect =
        playlist_rect.with_min_x(playlist_rect.min.x + TRACK_LABEL_WIDTH as f32);

    // Render currently present samples in the playlist
    // We should render the samples because when we are creating them we are also allocation responses
    // These responses would steal the input from the user if created after checking for input over the entire playlist.
    render_samples(
        this,
        ui,
        state,
        first_visible_track_idx,
        last_visible_track_idx,
        first_visible_beat,
        usable_playlist_rect,
        &track_lines,
        &beat_lines,
        &preferences,
        master_playback,
        global_state.clone(),
    );

    // We are going to have multiple layers of responses each capturing something different
    // Allocate a response for the entirety of the playlist
    // The main playlist response should capture scrolling input in order to offset the whole grid
    let ui_base = ui.allocate_rect(playlist_rect, Sense::hover());

    // If there is something dragged over the playlist preview the location of the sample
    hover_sample(
        ui,
        state,
        playlist_rect,
        &track_lines,
        &beat_lines,
        first_visible_track_idx,
        &ui_base,
        &preferences,
    );

    // Handle the sample if it is dropped into the playlist.
    drop_sample(
        this,
        ui,
        state,
        panels_state.clone(),
        &track_lines,
        &beat_lines,
        first_visible_track_idx,
        first_visible_beat,
        &ui_base,
    );

    let host = HOST_STATE.load();

    // Draw cursor on playlist
    draw_cursor(
        ui,
        usable_playlist_rect,
        grid_offset,
        calculate_beat_pos(
            *state.read().bpm.lock(),
            global_state
                .master_playback
                .sample_playback_tracker
                .load(std::sync::atomic::Ordering::Relaxed) as usize,
            host.sample_rate as usize,
            host.channel_count as usize,
        ) as f32
            * BEAT_WIDTH as f32,
        &preferences,
    );

    // Capture scroll if hovered
    if ui_base.hovered() {
        let scroll_delta = ui.input(|reader| reader.smooth_scroll_delta());
        state.write().grid_offset = grid_offset.add(scroll_delta * 200.).min(Vec2::default());
    }
}

fn render_samples(
    this: &Panel,
    ui: &mut Ui,
    state: &RwLock<PlaylistState>,
    before_first_visible_track_idx: usize,
    last_visible_track_idx: usize,
    first_visible_beat: usize,
    playlist_rect: Rect,
    track_lines: &[[Pos2; 2]],
    beat_lines: &[[Pos2; 2]],
    preferences: &PlaylistPreferences,
    _master_playback: Arc<MasterPlaybackThread>,
    global_state: GlobalState,
) {
    // Iterate over the samples and decide which one is in frame.
    let samples = state.read().samples.clone();

    let samples = samples.read().clone();

    // Iterate over all the positions
    for (pos, samples) in samples {
        // Iterate over the samples contained in the positions.
        for (sample_idx, sample) in samples.iter().enumerate() {
            // Check if the track is visible based on the track
            if !(pos.track >= before_first_visible_track_idx && pos.track <= last_visible_track_idx)
            {
                continue;
            }

            let line_idx = pos.beat as i64 - first_visible_beat as i64;

            let start_pos = if line_idx >= 0 && (line_idx as usize) < beat_lines.len() {
                beat_lines[line_idx as usize][0].x
            } else if !beat_lines.is_empty() {
                beat_lines[0][0].x + (line_idx as f32) * BEAT_WIDTH as f32
            } else {
                continue;
            };

            // Get track customization
            let _track_customization = get_track_customization(state, pos.track, preferences);

            // Calculate rectangle length
            let bps = *state.read().bpm.lock() / 60.;

            let rectangle_length =
                (symphonia::core::units::Time::from_millis(sample.properties.length as i64)
                    .as_secs() as f64
                    * bps
                    * BEAT_WIDTH as f64) as f32;

            // If the sample isn't long enough to reach onto the screen, skip it.
            if start_pos + rectangle_length < playlist_rect.left() {
                continue;
            }

            // Check if we have enough lines to display whatever we need (simple bounds checking to avoid panic)
            if track_lines.len() <= pos.track - before_first_visible_track_idx + 1 {
                continue;
            }

            // Create the rect where the sample might be rendered.
            let sample_rect = Rect::from_min_max(
                Pos2 {
                    x: start_pos,
                    y: (track_lines[pos.track - before_first_visible_track_idx][0].y),
                },
                Pos2 {
                    x: (start_pos + rectangle_length),
                    y: (track_lines[pos.track - before_first_visible_track_idx + 1][0].y),
                },
            );

            // Draw sample rect
            ui.painter()
                .with_clip_rect(playlist_rect)
                .rect_filled(sample_rect, 0., sample.color);

            // Create galley for sample label
            let galley = ui.fonts_mut(|f| {
                f.layout(
                    sample.name.clone(),
                    egui::FontId::proportional(12.0),
                    egui::Color32::WHITE,
                    sample_rect.width(),
                )
            });

            // Draw sample text
            ui.painter()
                .with_clip_rect(playlist_rect)
                .with_clip_rect(sample_rect)
                .galley(sample_rect.left_top(), galley.clone(), egui::Color32::WHITE);

            // Allocate a response over the sample to capture any inputs it receives
            let sample_response = ui.allocate_rect(sample_rect, Sense::all());

            // Draw the waveform of the sample
            let waveform_rect = sample_rect.shrink2(vec2(0., galley.rect.height()));

            // Only display the waveform if we actually have smth to display
            if let Some(waveform) = &sample.waveform_map {
                // Decide each columns width
                let column_width = waveform_rect.width() / waveform.len() as f32;

                let baseline_maximum_offset = waveform_rect.height() / 2.0;
                let middle_y = waveform_rect.top() + baseline_maximum_offset;

                // Fetch positions over sample
                let start = Pos2::new(waveform_rect.left(), middle_y);
                let end = Pos2::new(waveform_rect.right(), middle_y);

                // Draw a centerline serving as the indication for silence.
                ui.painter().with_clip_rect(playlist_rect).line(
                    [start, end].to_vec(),
                    Stroke::new(1.0_f32, preferences.waveform_color),
                );

                // Iter over all the samples and draw them
                // We are going to ratio this based on the highest/lowest value the output can get which is 1.0 and -1.0
                // There for the top of this rect is going to serve as 1.0 and the bottom is -1.0
                let mut idx = 0;
                let scale_reference = waveform
                    .iter()
                    .flat_map(|[min, max]| [min.abs(), max.abs()])
                    .fold(0.0_f32, f32::max)
                    .max(f32::EPSILON);

                // Draw all of the columns on the screen
                while idx < waveform.len() {
                    // The maximum values goes on top of the baseline and the minimum below it
                    let [min, max] = waveform[idx];

                    // The x coordinate we are operation on
                    let x_offset = column_width * idx as f32;

                    let x = waveform_rect.left() + x_offset;

                    // Starting location of the column
                    let baseline = Pos2::new(x, middle_y);

                    let normalized_max = max / scale_reference;
                    let normalized_min = min / scale_reference;

                    // Height of the column we are drawing
                    let height_max = -normalized_max * baseline_maximum_offset;
                    let height_min = -normalized_min * baseline_maximum_offset;

                    // Draw max
                    ui.painter().with_clip_rect(playlist_rect).line(
                        [baseline, Pos2::new(x, middle_y + height_max)].to_vec(),
                        Stroke::new(column_width, preferences.waveform_color),
                    );
                    // Draw min
                    ui.painter().with_clip_rect(playlist_rect).line(
                        [baseline, Pos2::new(x, middle_y + height_min)].to_vec(),
                        Stroke::new(column_width, preferences.waveform_color),
                    );

                    // Increment index
                    idx += 1;
                }
            }

            // If the sample is dragged, simulate a dnd again
            sample_response.dnd_set_drag_payload(sample.clone());

            // Remove the old position of the sample, if the list is empty or if it has been secondary clicked.
            // (By default this part only removes the node and when I create a dnd payload that can also insert it.)
            if sample_response.drag_stopped() || sample_response.secondary_clicked() {
                // Get handle to samples
                let read = state.read();
                let samples_handle = &mut *read.samples.write();

                // This will get set to true if the position does not contain any samples.
                // Check if the position contains any samples
                let should_be_deleted = if let Some(samples_at_pos) = samples_handle.get_mut(&pos) {
                    // Remove the sample from that position
                    samples_at_pos.remove(sample_idx);

                    samples_at_pos.is_empty()
                } else {
                    false
                };

                // Remove position if empty
                if should_be_deleted {
                    samples_handle.swap_remove(&pos);
                }
            }

            // Effects map of all the samples present in the playlist. (If they have one)
            let fx_map = global_state.fx_map.clone();

            // The unique id number to the sample
            let sample_id = sample.id;

            // Create a context menu for the node
            egui::Popup::menu(&sample_response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    // Display sample name
                    ui.label(RichText::from(&sample.name).weak());
                    ui.separator();

                    let is_fx_enabled = fx_map.contains_key(&sample_id);

                    // Display this portion of the ui
                    let fx_toggle = ui.checkbox(
                        &mut is_fx_enabled.clone(),
                        RichText::from("Effects").strong(),
                    );

                    if fx_toggle.clicked() {
                        if !is_fx_enabled {
                            fx_map.insert(sample_id, NodeMap::new());
                        } else {
                            fx_map.remove(&sample_id);
                        }
                    }

                    // This menubutton is deactivated until an effect map is created manually
                    ui.add_enabled_ui(fx_map.contains_key(&sample_id), |ui| {
                        // Create a menu button which displays the effects
                        ui.menu_button("Effects Chain", |ui| {
                            // Create desired size of the window
                            let desired_size = vec2(
                                ui.viewport_rect().width() / 3.,
                                ui.viewport_rect().height() / 3.,
                            );

                            // Allocate the ui so that it cannot automatically grow later.
                            ui.allocate_ui(desired_size, |ui| {
                                if let Some(mut fx_map) = fx_map.get_mut(&sample_id) {
                                    let fx_map = fx_map.value_mut();

                                    // Display effects chain map
                                    fx_map.display(ui);

                                    ui.separator();

                                    ui.horizontal(|ui| {
                                        ui.menu_button("Options", |ui| {
                                            ui.button("Fullscreen").clicked();
                                            if ui.button("Zoom In").clicked() {
                                                fx_map.ui_attributes.scale += 0.2;
                                            }

                                            if ui.button("Zoom Out").clicked() {
                                                fx_map.ui_attributes.scale -= 0.2;
                                            }

                                            ui.separator();

                                            if ui.button("Reset").clicked() {
                                                fx_map.reset();
                                            }
                                        });

                                        ui.separator();

                                        ui.menu_button("Plugins", |ui| {
                                            ui.menu_button("Builtin", |_ui| {});
                                            ui.menu_button("External", |ui| {
                                                let mut plugin_manager =
                                                    global_state.plugin_manager.write();

                                                for (path, plugin_handle) in
                                                    plugin_manager.loaded_plugins.clone().iter()
                                                {
                                                    if ui
                                                        .button(
                                                            path.file_name()
                                                                .unwrap_or_default()
                                                                .to_string_lossy(),
                                                        )
                                                        .clicked()
                                                    {
                                                        // Create an instance of the plugin
                                                        let plugin_instance =
                                                            plugin_handle.create_instance();

                                                        // Create the plugin's state buffer
                                                        let state = Arc::new(RwLock::new(
                                                            plugin_handle
                                                                .startup_memory_snapshot
                                                                .clone(),
                                                        ));

                                                        // Store the plugin's state buffer and instance ptr in a separate hashmap too for quick access on the state writer thread.
                                                        // We mustnt forget to update this list if any nodes or plugins get removed to avoid taking up too much memory.
                                                        plugin_manager.plugin_states.insert(
                                                            plugin_instance.plugin_instance_ptr
                                                                as usize,
                                                            (
                                                                plugin_instance.plugin_type,
                                                                state.clone(),
                                                            ),
                                                        );

                                                        // Create a node based on the plugin we added.
                                                        fx_map.push_node(Node::new(
                                                            NodeType::ExternalPlugin {
                                                                state,
                                                                plugin_instance:
                                                                    InstanceResult::new(
                                                                        plugin_instance,
                                                                    ),
                                                                plugin_descriptor:
                                                                    PluginDescriptor {
                                                                        path: path.clone(),
                                                                        plugin_type: plugin_handle
                                                                            .plugin_type,
                                                                    },
                                                            },
                                                            Pos2::default(),
                                                            [1, 0, 1],
                                                        ));
                                                    }
                                                }
                                            });
                                        });

                                        ui.separator();

                                        // Only try to display the options if there is a node selected.
                                        if let Some(node_id) = fx_map.currently_selected_node_id {
                                            let node = fx_map.get_node(node_id).clone();

                                            // Only display the remove button for nodes that can be removed.
                                            if node.node_type() != &NodeType::In
                                                && node.node_type() != &NodeType::Out
                                                && ui.button("Remove").clicked()
                                            {
                                                // Remove the node and its connections from the map
                                                let node = fx_map.remove_node(node_id);

                                                // Reset selected node id
                                                fx_map.currently_selected_node_id = None;

                                                match node.node_type() {
                                                    // This is unreachable
                                                    NodeType::In | NodeType::Out => (),
                                                    // Remove the plugin's fastpath from the states
                                                    NodeType::ExternalPlugin {
                                                        plugin_instance,
                                                        plugin_descriptor: _,
                                                        state: _,
                                                    } => {
                                                        // Close the plugin when removed
                                                        if let Ok(inst) = plugin_instance.get() {
                                                            display_error_as_toast(
                                                                inst.close(),
                                                                ToastStyle::default(),
                                                                this.toasts.clone(),
                                                            );

                                                            // Remove from state fastpath
                                                            global_state
                                                                .plugin_manager
                                                                .write()
                                                                .plugin_states
                                                                .remove(
                                                                    &(inst.plugin_instance_ptr
                                                                        as usize),
                                                                );
                                                        }
                                                    }
                                                    NodeType::InternalCustom(
                                                        _plugin_node_properties,
                                                    ) => todo!(),
                                                }
                                            }

                                            match node.node_type() {
                                                NodeType::In => {}
                                                NodeType::Out => {}
                                                NodeType::ExternalPlugin {
                                                    state,
                                                    plugin_instance,
                                                    ..
                                                } => {
                                                    if let Ok(instance) = plugin_instance.get() {
                                                        let is_closed = instance
                                                            .displayed_window_information
                                                            .try_lock()
                                                            .map(|inner| inner.is_none())
                                                            .unwrap_or(false);

                                                        ui.add_enabled_ui(is_closed, |ui| {
                                                            // Open the plugin by creating a window and providing that handle to the plugin's renderer.
                                                            if ui.button("Open").clicked() {
                                                                // Display plugin
                                                                // Load in the state of the plugin as we have stored it
                                                                display_error_as_toast(
                                                                    instance.open(
                                                                        state.clone(),
                                                                        node_id,
                                                                        sample_id,
                                                                    ),
                                                                    ToastStyle::default(),
                                                                    this.toasts.clone(),
                                                                );
                                                            }
                                                        });
                                                    }
                                                }
                                                NodeType::InternalCustom(
                                                    _plugin_node_properties,
                                                ) => {}
                                            }
                                        }
                                    });
                                }
                            });
                        });
                    });

                    // Draw a separator so that it looks more visually pleasing
                    ui.separator();

                    ui.menu_button("Properties", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Sample Rate");
                            ui.label(format!("{}hz", sample.properties.sample_rate));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Length");
                            ui.label(
                                NaiveTime::from_num_seconds_from_midnight_opt(
                                    (sample.properties.length as f64 / 1000.0 % 86400.0) as u32,
                                    0,
                                )
                                .unwrap_or_default()
                                .format("%H:%M:%S")
                                .to_string(),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Path");
                            ui.label(sample.path.display().to_string());
                        });

                        // Provide a bit more information id debug builds
                        #[cfg(debug_assertions)]
                        {
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("ID");
                                ui.label(sample.id.to_string());
                            });
                        }
                    });
                });

            // Allocate ui next to the name of the sample for quick access to the samples settings
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id(Id::new(sample.id).with(pos))
                    .max_rect(Rect {
                        min: Pos2::new(
                            sample_rect.left_top().x + galley.size().x + 20.,
                            sample_rect.left_top().y,
                        ),
                        max: Pos2::new(
                            sample_rect.right(),
                            sample_rect.left_top().y + galley.size().y,
                        ),
                    }),
                |ui| {
                    let sample_has_fx = fx_map.contains_key(&sample_id);

                    ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                        // Create an image that indicates whether effects are enabled for this specific sample
                        egui::Image::new(egui::include_image!("..\\..\\..\\assets\\fxw.svg"))
                            .tint(if sample_has_fx {
                                Color32::LIGHT_BLUE
                            } else {
                                Color32::WHITE
                            })
                            .max_size(Vec2::new(galley.size().y, galley.size().y))
                            .ui(ui)
                            .on_hover_text(if sample_has_fx {
                                "This sample has effects enabled."
                            } else {
                                "This sample does not have any effects applied."
                            });
                    });
                },
            );
        }
    }
}

fn drop_sample(
    this: &Panel,
    ui: &mut Ui,
    state: &RwLock<PlaylistState>,
    global_state: Arc<PanelStates>,
    track_lines: &[[Pos2; 2]],
    beat_lines: &[[Pos2; 2]],
    first_visible_track_idx: usize,
    first_visible_beat: usize,
    ui_base: &egui::Response,
) {
    if let Some(payload) = ui_base.dnd_release_payload::<SampleInstance>() {
        let id = state.read().get_sample_count();

        // Get cursor position
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            // Find starting beat position on the x axis (index into beat_lines)
            let (_, relative_beat_pos) =
                find_value_inbetween(beat_lines.iter().map(|v| v[0].x), cursor.x)
                    .unwrap_or_default();

            // Find starting beat position on the y axis
            let (_, relative_track_pos) =
                find_value_inbetween(track_lines.iter().map(|v| v[0].y), cursor.y)
                    .unwrap_or_default();

            // We have to subtract one from the relative position since the first track's position is out of bounds (its the topmost line of the whole playlist)
            let absolute_track_idx = first_visible_track_idx + relative_track_pos - 1;
            let absolute_beat_pos = relative_beat_pos.max(1) - 1 + first_visible_beat;

            // If anything gets dropped into the "workspace" aka the playlist then add it to the workspace files
            // Look up if we have already stored this one sample
            let query = global_state
                .media_panel
                .read()
                .workspace_selector
                .workspace_samples
                .get(&payload.path)
                .cloned();

            // Check if we already have this sample in the workspace tab
            let sample_instance = if let Some(sample_info) = query {
                global_state
                    .media_panel
                    .write()
                    .workspace_selector
                    .workspace_samples
                    .get(&payload.path);

                // If we do have this sample then insert into playlist accordingly
                SampleInstance {
                    id,
                    name: sample_info.alias.clone(),
                    color: {
                        // If the color of this sample has been modified, the new color should be displayed when reinserted.
                        if payload.color != sample_info.color {
                            payload.color
                        } else {
                            sample_info.color
                        }
                    },
                    path: payload.path.clone(),
                    properties: payload.properties.clone(),
                    waveform_map: sample_info.waveform_map,
                }
            }
            // Initalize new sample in workspace
            // Generate a new random color for it
            else {
                this.toasts.lock().add(
                    Toast::new()
                        .kind(egui_toast::ToastKind::Info)
                        .text(format!("Imported sample `{}`", payload.name)),
                );

                // Map the waveforms of the sample if it hadnt been inserted yet
                let waveform_map = display_error_as_toast(
                    generate_sample_waveform(&payload.path),
                    ToastStyle::default(),
                    this.toasts.clone(),
                );

                let random_color = random_color_with_opacity(120);

                global_state
                    .media_panel
                    .write()
                    .workspace_selector
                    .workspace_samples
                    .insert(
                        payload.path.clone(),
                        WorkspaceSampleAttributes {
                            alias: payload.name.clone(),

                            // All samples have their color synced by default.
                            is_color_synced: true,
                            color: random_color,
                            waveform_map: waveform_map.clone(),
                        },
                    );

                SampleInstance {
                    id,
                    name: payload.name.clone(),
                    color: random_color,
                    path: payload.path.clone(),
                    properties: payload.properties.clone(),
                    waveform_map,
                }
            };

            // Create the position instance of the sample
            let sample_position = Position {
                track: absolute_track_idx,
                beat: absolute_beat_pos,
            };

            let read = state.read();
            let samples_handle = &mut *read.samples.write();

            // If there are already samples at that location we can append this sample to that specific location
            if let Some(samples) = samples_handle.get_mut(&sample_position) {
                // Store sample
                samples.push(sample_instance);
            }
            // If there are no samples in that location create a list containing that sample.
            // Make sure to clean up the list if it is empty.
            else {
                // Store sample in playlist
                samples_handle.insert(sample_position, vec![sample_instance.clone()]);
            }
        }
    }
}

fn hover_sample(
    ui: &mut Ui,
    state: &RwLock<PlaylistState>,
    playlist_rect: Rect,
    track_lines: &[[Pos2; 2]],
    beat_lines: &[[Pos2; 2]],
    first_visible_track_idx: usize,
    ui_base: &egui::Response,
    preferences: &PlaylistPreferences,
) {
    if let Some(payload) = ui_base.dnd_hover_payload::<SampleInstance>() {
        // Get cursor position
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            // Find starting beat position on the x axis
            let (starting_x, _relative_beat_pos) =
                find_value_inbetween(beat_lines.iter().map(|v| v[0].x), cursor.x)
                    .unwrap_or_default();

            // Find starting beat position on the y axis
            let (starting_y, relative_track_pos) =
                find_value_inbetween(track_lines.iter().map(|v| v[0].y), cursor.y)
                    .unwrap_or_default();

            // We have to subtract one from the relative position since the first track's position is out of bounds (its the topmost line of the whole playlist)
            let absolute_track_idx = first_visible_track_idx + relative_track_pos - 1;

            // Clamp both x and y for the preview to draw correctly.
            let starting_x = starting_x.max(playlist_rect.left() + TRACK_LABEL_WIDTH as f32);
            let starting_y = starting_y.max(playlist_rect.top());

            // Fetch track attributes
            let track_customization =
                get_track_customization(state, absolute_track_idx, preferences);

            // Calculate rectangle length
            let bps = *state.read().bpm.lock() / 60.;

            // This is basically secs / bps * beat_width
            let rectangle_length =
                symphonia::core::units::Time::from_millis(payload.properties.length as i64)
                    .as_secs() as f64
                    * bps
                    * BEAT_WIDTH as f64;

            if relative_track_pos >= track_lines.len() {
                return;
            }

            let rect_points = [
                Pos2::new(starting_x, starting_y),
                Pos2::new(
                    (starting_x + rectangle_length as f32).min(playlist_rect.right()),
                    (starting_y + track_customization.height)
                        .min(track_lines[relative_track_pos][0].y),
                ),
            ];

            // Draw the rectangle indicating how long the sample is
            ui.painter()
                .rect_filled(Rect::from_points(&rect_points), 0., payload.color);
        }
    }
}

fn get_track_customization(
    state: &RwLock<PlaylistState>,
    idx: usize,
    preferences: &PlaylistPreferences,
) -> TrackCustomization {
    let read = state.read();
    match read.custom_tracks.get(&idx) {
        Some(custom) => custom.clone(),
        None => TrackCustomization::named_default(
            idx,
            preferences.default_track_label_color,
            preferences.default_track_label_text_color,
        ),
    }
}

/// Draws main cursor (Indicates where we are in current playlist)
fn draw_cursor(
    ui: &mut Ui,
    usable_playlist_rect: Rect,
    grid_offset: Vec2,
    cursor_offset: f32,
    preferences: &PlaylistPreferences,
) {
    ui.painter().line(
        vec![
            Pos2::new(
                (usable_playlist_rect.left() + cursor_offset + grid_offset.x)
                    .min(usable_playlist_rect.right()),
                usable_playlist_rect.top(),
            ),
            Pos2::new(
                (usable_playlist_rect.left() + cursor_offset + grid_offset.x)
                    .min(usable_playlist_rect.right()),
                usable_playlist_rect.bottom(),
            ),
        ],
        Stroke::new(STROKE_WIDTH, preferences.cursor_color),
    );
}

/// Draws beat outlines from the left of the playlist to the right with the step of `beat_width`.
fn beat_outlines(
    ui: &mut Ui,
    playlist_rect: Rect,
    x_offset_ratio: f32,
    beat_width: f32,
) -> (usize, Vec<[Pos2; 2]>) {
    let mut line_positions = Vec::new();

    // The position of "beat 0" (first beat after the label region) with no scroll applied.
    let label_end = playlist_rect.left() + beat_width * 4.0;

    // Shift by the scroll offset to find where beat 0 currently sits on screen.
    let beat_zero_x = label_end + x_offset_ratio;

    // How many whole beats have scrolled past beat 0 (positive = scrolled right).
    let beats_past_zero = ((label_end - beat_zero_x) / beat_width).ceil().max(0.0);

    let first_visible_beat = beats_past_zero as usize;
    let mut x_coord = beat_zero_x + beats_past_zero * beat_width;

    while x_coord <= playlist_rect.right() {
        let line_pos = [
            Pos2::new(x_coord, playlist_rect.top()),
            Pos2::new(x_coord, playlist_rect.bottom()),
        ];
        ui.painter().line(
            line_pos.to_vec(),
            Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
        );

        // Store the line position
        line_positions.push(line_pos);

        x_coord += beat_width;
    }

    (first_visible_beat, line_positions)
}

fn track_label<'a>(
    ui: &mut Ui,
    playlist_rect: Rect,
    idx: usize,
    label_customization: &TrackCustomization,
    top: f32,
    bottom: f32,
    custom_tracks: &mut HashMap<usize, TrackCustomization>,
    preferences: &PlaylistPreferences,
) {
    let label_rect = Rect::from_two_pos(
        Pos2 {
            x: playlist_rect.left(),
            y: top,
        },
        Pos2 {
            x: playlist_rect.left() + TRACK_LABEL_WIDTH as f32,
            y: bottom,
        },
    );

    // Draw the label itself
    ui.painter()
        .rect_filled(label_rect, 0., label_customization.label_color);

    // Draw the label text
    ui.painter().text(
        label_rect.center(),
        Align2::CENTER_TOP,
        label_customization.label_text.clone(),
        FontId::default(),
        label_customization.label_text_color,
    );

    // Allocate the response for the given track
    let label = ui.allocate_rect(label_rect, Sense::click());

    // Detect if it has been right clicked on and store a entry in the customization list.
    if label.secondary_clicked() && !custom_tracks.contains_key(&idx) {
        custom_tracks.insert(
            idx,
            TrackCustomization::named_default(
                idx,
                preferences.default_track_label_color,
                preferences.default_track_label_text_color,
            ),
        );
    }

    // We should only allow the context menu to be opened if we already have the track customizations saved in the list.
    if custom_tracks.contains_key(&idx) {
        // Get mutable access to the created item
        // Its safe to unwrap here due to the check above
        let customization_state = custom_tracks.get_mut(&idx).unwrap();
        // Open ctx menu and access the entry weve created
        let popup = egui::Popup::context_menu(&label)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);

        let ctx_menu = popup.show(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::from("Label").weak());
                ui.text_edit_singleline(&mut customization_state.label_text);
            });
            ui.separator();
            ui.label(RichText::from("Label Color"));
            egui::widgets::color_picker::color_picker_color32(
                ui,
                &mut customization_state.label_color,
                egui::widgets::color_picker::Alpha::Opaque,
            );
            ui.separator();
            ui.label(RichText::from("Label Text Color"));
            egui::widgets::color_picker::color_picker_color32(
                ui,
                &mut customization_state.label_text_color,
                egui::widgets::color_picker::Alpha::Opaque,
            );
        });

        // Check if the user has clicked outside of the context menu
        if ctx_menu.is_none() {
            // If the context menu is closed we should check if the customization entry has been modified
            // If not just remove it to save up memory
            if *customization_state
                == TrackCustomization::named_default(
                    idx,
                    preferences.default_track_label_color,
                    preferences.default_track_label_text_color,
                )
            {
                custom_tracks.remove(&idx);
            }
        }
    }
}

fn track_separator(
    ui: &mut Ui,
    playlist_rect: Rect,
    normalized_y_offset: f32,
    idx: usize,
    y_coord: f32,
    label_customization: &TrackCustomization,
    custom_tracks: &mut HashMap<usize, TrackCustomization>,
    preferences: &PlaylistPreferences,
) -> [Pos2; 2] {
    // Draw track separator lines
    let separator_points = [
        Pos2::new(
            playlist_rect.left(),
            (y_coord + normalized_y_offset + label_customization.height)
                .clamp(playlist_rect.top(), playlist_rect.bottom()),
        ),
        Pos2::new(
            playlist_rect.right(),
            (y_coord + normalized_y_offset + label_customization.height)
                .clamp(playlist_rect.top(), playlist_rect.bottom()),
        ),
    ];

    ui.painter().line(
        separator_points.to_vec(),
        Stroke::new(STROKE_WIDTH, BAR_TRACK_SEPARATOR),
    );

    // Allocate a response for being able to set the height of the tracks
    let separator = ui.allocate_rect(
        Rect::from_points(&separator_points).expand2(vec2(0., 2.5)),
        Sense::click_and_drag(),
    );

    // Get how much this has been dragged by
    let height_delta = separator.drag_delta().y;
    let pixel_delta = ui.pixels_per_point() * height_delta;

    // Check if a drag has been started
    if separator.drag_started() && !custom_tracks.contains_key(&idx) {
        custom_tracks.insert(
            idx,
            TrackCustomization::named_default(
                idx,
                preferences.default_track_label_color,
                preferences.default_track_label_text_color,
            ),
        );
    }

    // Check if the item is inside the list
    if custom_tracks.contains_key(&idx) {
        // Get mutable access to the created item
        // Its safe to unwrap here due to the check above
        let customization_state = custom_tracks.get_mut(&idx).unwrap();

        // Set that it has been modified already
        customization_state.height_set = true;

        // If it has been double clicked that means that it should minimize the track or if its already minimzed then reset it to the original value
        if separator.double_clicked() {
            if customization_state.height != MINIMUM_TRACK_HEIGHT {
                customization_state.height = MINIMUM_TRACK_HEIGHT;
            } else {
                customization_state.height = TRACK_HEIGHT;
            }
        } else {
            customization_state.height = customization_state
                .height
                .add(pixel_delta)
                .max(MINIMUM_TRACK_HEIGHT);
        }
    }

    // Indicate that this can be grabbed
    separator.on_hover_cursor(egui::CursorIcon::ResizeVertical);

    separator_points
}
