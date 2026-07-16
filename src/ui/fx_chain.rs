use std::{collections::HashSet, path::PathBuf};

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2, vec2};
use strum::{EnumCount, VariantArray};

use crate::plugins::PluginHandle;

#[derive(Debug, Clone, Copy)]
/// The attributes of an object in the Ui.
pub struct UiAttributes {
    /// How far are we zoomed in. (2.0 => 2x)
    scale: f32,

    /// How much the user has dragged the whole map.
    offset: Vec2,
}

impl Default for UiAttributes {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset: Vec2::default(),
        }
    }
}

pub struct NodeMap {
    /// The nodes which are contained by the map.
    /// When referring to a node's id we are referring to its index in this list.
    nodes: Vec<Node>,

    /// The attributes of this [`NodeMap`] in the Ui.
    pub ui_attributes: UiAttributes,

    /// The connections between the nodes.
    /// This is vital for the creation of the effects chain.
    /// Dont forget to call `make_connection` on the two [`ConnectorID`]-s we are planning to insert so that order wont matter.
    pub node_connections: HashSet<[ConnectorID; 2]>,

    /// The currently selected node's id.
    /// This is used to edit or remove a node from the map.
    pub currently_selected_node_id: Option<usize>,

    pub currently_selected_connector: Option<ConnectorID>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    // ------------
    // These two implemented so that we can always order the ConnectorIDs present in a connection so that connector order doesnt matter.
    // Do not forget to call `create_connection` every time a connection is inserted into `node_connections`
    // ------------

    // ------------
    PartialOrd,
    Ord,
    // ------------
)]
pub struct ConnectorID {
    pub node_id: usize,
    pub side: Side,
    pub connector_idx: usize,
    pub connector_count: usize,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, strum::EnumCount, strum::VariantArray,
)]
pub enum Side {
    Left = 0,
    Right = 1,
    Bottom = 2,
}

impl Side {
    pub fn to_color(&self) -> Color32 {
        match self {
            Side::Left => Color32::BLUE,
            Side::Right => Color32::RED,
            Side::Bottom => Color32::WHITE,
        }
    }
}

fn create_connection([a, b]: [ConnectorID; 2]) -> [ConnectorID; 2] {
    if a <= b { [a, b] } else { [b, a] }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginNodeProperties {}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    /// Main sample in.
    /// This is where the (resampled) original samples flow into the map.
    In,

    /// Main sample out.
    /// This is where the final samples flow out of the map after all the effects (if any) have been applied to them.
    Out,

    /// Plugin node.
    /// This node manages the underlying VST plugin's effects on the samples in the effects chain.
    ExternalPlugin {
        path: PathBuf,
    },

    /// Internal plugin node.
    /// These are gonna be more customizable since these are directly integrated into the application.
    /// Idea:
    /// I should make a channel decoupler plugin which separates the channels into N outputs.
    InternalCustom(PluginNodeProperties),
}

/// Size of a connector's own box.
const CONNECTOR_SIZE: f32 = 20.0;
/// Gap between adjacent connectors.
const CONNECTOR_GAP: f32 = 10.0;

#[derive(Debug, Clone)]
pub struct Node {
    /// The type of this node.
    /// This could be a custom (user made) node - or the default in or out nodes.
    node_type: NodeType,

    /// The position of the node in the nodemap.
    position: Pos2,

    /// Shows the number of connectors on each side of the node.
    /// A connectors size is 20.0 in every direction.
    /// The amount of sides may change in the future, but for now treat it as 3 (left, right, bottom).
    /// When creating the array of numbers containing the number of connectors on the nodes sides the directions' order follow as: `left, right, bottom`.
    connectors: [usize; Side::COUNT],

    /// The size of this node.
    /// This is calculated when the node is created. (Calulated by the maximum amount of connectors on either its left or right side and bottom.)
    /// The node may be resized if there are multiple connectors on its side.
    size: Vec2,
}

impl Node {
    pub fn new(node_type: NodeType, position: Pos2, connectors: [usize; Side::COUNT]) -> Self {
        Self {
            node_type,
            position,
            size: Node::calculate_size(connectors),
            connectors,
        }
    }

    /// Calculates the size of the node based on its connectors.
    pub fn calculate_size(connectors: [usize; Side::COUNT]) -> Vec2 {
        vec2(
            80.0 + (connectors[2] as f32 * (CONNECTOR_SIZE + 10.0)),
            //
            25.0 + (connectors[0].max(connectors[1]) as f32 * (CONNECTOR_SIZE + 10.0)),
        )
    }

    pub fn node_type(&self) -> &NodeType {
        &self.node_type
    }
}

impl NodeMap {
    /// Constructs an empty nodemap.
    pub fn new() -> Self {
        Self {
            // Create the two default nodes in every effects chain.
            nodes: Vec::from([
                Node::new(
                    // Set the type of this node.
                    NodeType::In,
                    Pos2::new(-300.0, 0.),
                    [0, 1, 0],
                ),
                Node::new(
                    // Set the type of this node.
                    NodeType::Out,
                    Pos2::new(300., 0.),
                    [1, 0, 0],
                ),
            ]),
            ui_attributes: UiAttributes::default(),
            currently_selected_node_id: None,
            currently_selected_connector: None,

            // By default the output and the input should be connected.
            node_connections: HashSet::from([[
                ConnectorID {
                    node_id: 0,
                    side: Side::Right,
                    connector_count: 1,
                    connector_idx: 0,
                },
                ConnectorID {
                    node_id: 1,
                    side: Side::Left,
                    connector_count: 1,
                    connector_idx: 0,
                },
            ]]),
        }
    }

    /// Displays the nodemap in the ui provided.
    pub fn display(&mut self, ui: &mut egui::Ui) {
        let desired_size = ui.available_size();
        ui.set_min_size(desired_size);
        ui.set_max_size(desired_size);

        let available_rect = ui.max_rect();

        // This will serve as the center point of out map.
        let reference_point = Pos2::new(
            available_rect.left() + available_rect.width() / 2.,
            available_rect.top() + available_rect.height() / 2.,
        );

        // Display the background of the map
        self.display_background(ui, available_rect, reference_point);

        // Allocate the response for the background so that it becomes draggable but other nodes can steal the input.
        let bg_drag = ui.allocate_rect(available_rect, Sense::drag());
        self.ui_attributes.offset += bg_drag.drag_delta();

        self.draw_nodes(ui, available_rect, reference_point);
        self.draw_unfinished_connection_to_cursor(ui, available_rect, reference_point);
        self.draw_connections(ui, available_rect, reference_point);

        // Allocate the respones for the background's zoom so that the map will always be able to resize.
        let bg = ui.allocate_rect(available_rect, Sense::hover());
        if bg.hovered() {
            let scroll_delta = ui.input(|reader| reader.smooth_scroll_delta()).y / 1.5;

            self.ui_attributes.scale =
                (self.ui_attributes.scale + scroll_delta * 0.01).clamp(0.3, 5.0);
        }
    }

    fn draw_unfinished_connection_to_cursor(
        &mut self,
        ui: &mut egui::Ui,
        available_rect: egui::Rect,
        reference_point: Pos2,
    ) {
        // If there is an ongoing dragged connector then preview the line to the cursor
        if let Some(connector) = &self.currently_selected_connector {
            let connector_node = &self.nodes[connector.node_id];

            // Fetch the connector's pos which has been selected
            let connector_pos = calculate_connector_pos(
                egui::Rect::from_center_size(
                    connector_node.position * self.ui_attributes.scale + self.ui_attributes.offset,
                    connector_node.size * self.ui_attributes.scale,
                ),
                connector.side,
                connector.connector_idx,
                connector.connector_count,
                self.ui_attributes.scale,
            ) + reference_point.to_vec2();

            // Draw the line to the cursor
            if let Some(pointer_pos) = ui.input(|reader| reader.pointer.latest_pos()) {
                ui.painter().with_clip_rect(available_rect).line(
                    [connector_pos, pointer_pos].to_vec(),
                    Stroke::new(1.0_f32, Color32::WHITE),
                );
            }
        }
    }

    /// This function draws the lines between the nodes for the connections. It does not verify the validness of the connections.
    fn draw_connections(
        &self,
        ui: &mut egui::Ui,
        available_rect: egui::Rect,
        reference_point: Pos2,
    ) {
        // Draw the connections between the nodes
        for [lhs, rhs] in &self.node_connections {
            if self.nodes.get(lhs.node_id).is_none() || self.nodes.get(rhs.node_id).is_none() {
                eprintln!(
                    "Invalid node connection between node index {}->{}",
                    lhs.node_id, rhs.node_id
                );

                continue;
            }

            // Get each node where its coming from
            let lhs_node = &self.nodes[lhs.node_id];
            let rhs_node = &self.nodes[rhs.node_id];

            // Paint the lines themselves
            let points = [
                calculate_connector_pos(
                    egui::Rect::from_center_size(
                        lhs_node.position * self.ui_attributes.scale + self.ui_attributes.offset,
                        lhs_node.size * self.ui_attributes.scale,
                    ),
                    lhs.side,
                    lhs.connector_idx,
                    lhs.connector_count,
                    self.ui_attributes.scale,
                ) + reference_point.to_vec2(),
                calculate_connector_pos(
                    egui::Rect::from_center_size(
                        rhs_node.position * self.ui_attributes.scale + self.ui_attributes.offset,
                        rhs_node.size * self.ui_attributes.scale,
                    ),
                    rhs.side,
                    rhs.connector_idx,
                    rhs.connector_count,
                    self.ui_attributes.scale,
                ) + reference_point.to_vec2(),
            ]
            .to_vec();

            ui.painter()
                .with_clip_rect(available_rect)
                .line(points, Stroke::new(1.0_f32, Color32::WHITE));
        }
    }

    fn draw_nodes(&mut self, ui: &mut egui::Ui, available_rect: egui::Rect, reference_point: Pos2) {
        // Draw the nodes themselves
        for (node_id, node) in self.nodes.clone().iter().enumerate() {
            // Draw the actual nodes themselves
            let center = Pos2::new(
                reference_point.x
                    + node.position.x * self.ui_attributes.scale
                    + self.ui_attributes.offset.x,
                reference_point.y
                    + node.position.y * self.ui_attributes.scale
                    + self.ui_attributes.offset.y,
            );

            let node_rect =
                egui::Rect::from_center_size(center, node.size * self.ui_attributes.scale);

            // Draw the body and the outline of the node
            // The outline would only be visible if two nodes overlap.
            ui.painter().with_clip_rect(available_rect).rect(
                node_rect,
                1.,
                // Fill the rect of the node with the specified color
                {
                    if self.currently_selected_node_id == Some(node_id) {
                        Color32::GOLD
                    } else {
                        Color32::DARK_GRAY
                    }
                },
                Stroke::new(1.0, Color32::BLACK),
                egui::StrokeKind::Outside,
            );

            // Create galley for sample label
            let galley = ui.fonts_mut(|f| {
                f.layout(
                    match &node.node_type {
                        NodeType::In => "Input".to_string(),
                        NodeType::Out => "Output".to_string(),
                        NodeType::ExternalPlugin { path, .. } => path.to_string_lossy().to_string(),
                        NodeType::InternalCustom(_) => "Built-in".to_string(),
                    },
                    egui::FontId::proportional(10.0 * self.ui_attributes.scale),
                    // Display the label of the node with the specified color
                    {
                        if self.currently_selected_node_id == Some(node_id) {
                            Color32::BLACK
                        } else {
                            Color32::WHITE
                        }
                    },
                    node_rect.width(),
                )
            });

            // Calculate center of text
            let text_pos = center - galley.size() / 2.0;

            // Draw the name of the node
            ui.painter()
                .with_clip_rect(available_rect)
                .galley(text_pos, galley, Color32::WHITE);

            let node_ui_id = ui.id().with(("node", node_id));
            let node_response = ui.interact(node_rect, node_ui_id, Sense::click_and_drag());

            if node_response.dragged() {
                // drag_delta() is in screen pixels; divide by scale to convert
                // that screen-space movement into the node's local coordinate space.
                self.nodes[node_id].position +=
                    node_response.drag_delta() / self.ui_attributes.scale;
            }

            // If the node was clicked on save it as selected.
            // The user can manage it from anywhere else.
            if node_response.clicked() {
                // If the user clicked on the same node again de-select the node
                if self.currently_selected_node_id == Some(node_id) {
                    self.currently_selected_node_id = None;
                } else {
                    // Select the node if this is the first click on this node.
                    self.currently_selected_node_id = Some(node_id);
                }
            }

            // Match the nodes type and do something based on that.
            match &node.node_type {
                // Samples coming in, left side of the screen.
                NodeType::In => {
                    // Display information if hovered
                    node_response.on_hover_text("This is the main `In` node. This is the starting point of the samples' pipeline in the effects chain.");
                }
                // Samples going out, right side of the screen. (normally)
                NodeType::Out => {
                    // Display information if hovered
                    node_response.on_hover_text("This is the main `Out` node. This is the end point of the samples' pipleline in the effects chain. The information that enters this node gets sent to the mixer.");
                }
                NodeType::ExternalPlugin { .. } => {}
                NodeType::InternalCustom(_props) => {
                    // Create the connectors for a node with any number of connectors
                }
            };

            // Draw connectors and sense clicks on the connectors
            let mut clicked_connector: Option<ConnectorID> = None;

            // Iter over all the connectors and try to see if there was a click
            // All of the connectors space (width or height) have been pre-allocated (by default)
            for (idx, direction) in Side::VARIANTS.iter().enumerate() {
                // Get the number of connectors on this side
                let connector_count = node.connectors[idx];

                for connector_idx in 0..connector_count {
                    // Create the connector_id instance
                    let current_connector_id = ConnectorID {
                        node_id,
                        side: *direction,
                        connector_idx,
                        connector_count,
                    };

                    // Fetch the position of the connector and create a rect at the position
                    let connector_pos = calculate_connector_pos(
                        node_rect,
                        *direction,
                        connector_idx,
                        connector_count,
                        self.ui_attributes.scale,
                    );
                    let connector_rect = Rect::from_center_size(
                        connector_pos,
                        Vec2::new(CONNECTOR_SIZE, CONNECTOR_SIZE) * self.ui_attributes.scale,
                    );

                    // Draw the connector with the correct color
                    ui.painter().with_clip_rect(available_rect).rect_filled(
                        connector_rect,
                        0.,
                        direction.to_color(),
                    );

                    let connector_ui_id = ui.id().with(("connector", node_id, idx, connector_idx));

                    // Allocate a response at the rect
                    let connector = ui.interact(connector_rect, connector_ui_id, Sense::click());

                    // If the connector was clicked set the appropriate variable
                    if connector.clicked() {
                        // Remove the connector from every connection its mentioned in, so that one connector can only have one connection
                        self.remove_connector_id(current_connector_id);

                        // Save the clicked connector
                        clicked_connector = Some(current_connector_id)
                    }

                    // If the connector was right clicked remove the connector from the list
                    if connector.secondary_clicked() {
                        self.remove_connector_id(current_connector_id);
                    }
                }
            }

            // If a connector was clicked try to make a connection
            if let Some(clicked_connector) = clicked_connector {
                match self.currently_selected_connector {
                    Some(selected) => {
                        // Check if we are not trying to short circuit the path. (ie connecting a node to intself)
                        if selected.node_id != clicked_connector.node_id {
                            // Insert only if its correct
                            self.node_connections
                                .insert(create_connection([selected, clicked_connector]));

                            // Only reset the currently dragged if we actually inserted smth
                            self.currently_selected_connector = None;
                        }
                        // If the user clicked on the same connector reset the selected connector.
                        else if selected == clicked_connector {
                            self.currently_selected_connector = None;
                        }
                    }
                    None => {
                        // If there are no selected node connectors then select the current one.
                        self.currently_selected_connector = Some(clicked_connector);
                    }
                }
            }
        }
    }

    fn remove_connector_id(&mut self, current_connector_id: ConnectorID) {
        self.node_connections.retain(|connection| {
            connection[0] != current_connector_id && connection[1] != current_connector_id
        });
    }

    fn display_background(
        &self,
        ui: &mut egui::Ui,
        available_rect: egui::Rect,
        reference_point: Pos2,
    ) {
        // Display a black background in the available ui
        ui.painter().rect_filled(available_rect, 5., Color32::BLACK);

        // Display dots in the background with the given scaling every 15px
        let spacing = 15.0 * self.ui_attributes.scale;

        // Get maximum y coordinate
        let max_y = available_rect.bottom();
        // Get maximum x coordinate
        let max_x = available_rect.right();

        let origin = reference_point + self.ui_attributes.offset;

        let start_x = origin.x + ((available_rect.left() - origin.x) / spacing).floor() * spacing;
        let start_y = origin.y + ((available_rect.top() - origin.y) / spacing).floor() * spacing;

        let mut y_coord = start_y;

        // Iter over all the y coordinates
        while y_coord < max_y {
            // Iterate over the x coordinates
            let mut x_coord = start_x;

            while x_coord < max_x {
                // Draw dots to make it look more pleasing to the eye
                ui.painter().with_clip_rect(available_rect).circle_filled(
                    Pos2::new(x_coord, y_coord),
                    1.,
                    // A shade darker than gray
                    Color32::GRAY,
                );

                // Increment x coordinate
                x_coord += spacing;
            }

            // Increment y coordinate
            y_coord += spacing;
        }
    }

    pub fn remove_node(&mut self, id: usize) {
        // Remove the node from the Nodes list
        self.nodes.swap_remove(id);

        // Remove every connection which contains this node that was removed.
        self.node_connections.retain(|[lhs, rhs]| {
            if lhs.node_id == id || rhs.node_id == id {
                false
            } else {
                true
            }
        });
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn get_node(&self, idx: usize) -> &Node {
        &self.nodes[idx]
    }

    pub fn push_node(&mut self, value: Node) {
        self.nodes.push(value)
    }
}

fn calculate_connector_pos(
    node_rect: egui::Rect,
    side: Side,
    connector_idx: usize,
    connector_count: usize,
    scale: f32,
) -> Pos2 {
    debug_assert!(connector_count > 0, "connector_count must be > 0");
    debug_assert!(
        connector_idx < connector_count,
        "connector_idx {connector_idx} out of bounds for connector_count {connector_count}"
    );

    // Scale the layout constants to match the (already-scaled) node_rect.
    let connector_size = CONNECTOR_SIZE * scale;
    let connector_gap = CONNECTOR_GAP * scale;
    let connector_step = connector_size + connector_gap;

    // Total length occupied by all connectors + gaps between them (no trailing gap).
    let total_span =
        connector_count as f32 * connector_size + (connector_count as f32 - 1.0) * connector_gap;

    match side {
        Side::Left | Side::Right => {
            let start = (node_rect.height() - total_span) / 2.0;
            let offset = start + connector_idx as f32 * connector_step + connector_size / 2.0;

            let x = if side == Side::Right {
                node_rect.right()
            } else {
                node_rect.left()
            };
            Pos2::new(x, node_rect.top() + offset)
        }
        Side::Bottom => {
            let start = (node_rect.width() - total_span) / 2.0;
            let offset = start + connector_idx as f32 * connector_step + connector_size / 2.0;

            Pos2::new(node_rect.left() + offset, node_rect.bottom())
        }
    }
}
