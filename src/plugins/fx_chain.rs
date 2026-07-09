use std::collections::{HashSet, LinkedList};

use egui::{Align2, AtomExt, Color32, Pos2, RichText, Sense, Stroke, Vec2, vec2};

use crate::plugins::fx_chain::ConnectorSide::{Left, Right};

pub struct UiAttributes {
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
    ui_attributes: UiAttributes,

    /// The currently selected node's id.
    /// This is used to edit or remove a node from the map.
    currently_selected_node_id: Option<usize>,
}

#[derive(Debug, PartialEq)]
pub enum ConnectorSide {
    Left,
    Right,
}

#[derive(Debug, PartialEq, Clone)]
/// A node connector indicates what node id it is connected to.
pub enum NodeConnector {
    Single(usize),
    Multiple(Vec<usize>),
}

pub enum NodeType {
    /// Main sample in.
    /// This is where the (resampled) original samples flow into the map.
    In,

    /// Main sample out.
    /// This is where the final samples flow out of the map after all the effects (if any) have been applied to them.
    Out,

    /// Plugin node.
    /// This node manages the underlying VST plugin's effects on the samples in the effects chain.
    Plugin,
}

pub struct Node {
    /// The type of this node.
    /// This could be a custom (user made) node - or the default in or out nodes.
    node_type: NodeType,

    /// The connection this node has to others. (Please note that a node does not have to have all its connectors populated.)
    node_connection: NodeConnection,
    
    /// The position of the node in the nodemap.
    position: Pos2,

    /// The size of this node.
    /// The node may be resized if there are multiple connectors on its side.
    size: Vec2,
}

/// All nodes can have one or more connectors on its left and right side.
/// The first item in this array serves as the left and the second as the right side of the node.
pub type NodeConnection = [Option<NodeConnector>; 2];

impl Node {
    pub fn new(
        node_type: NodeType,
        position: Pos2,
        size: Vec2,
        node_connection: NodeConnection,
    ) -> Self {
        Self {
            node_type,
            position,
            size,
            node_connection,
        }
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
                    vec2(80., 25.),
                    // This node (main in) is connected to the 2nd item of this default list (idx 1) by default.
                    [None, Some(NodeConnector::Single(1))],
                ),
                Node::new(
                    // Set the type of this node.
                    NodeType::Out,
                    Pos2::new(300., 0.),
                    vec2(80., 25.),
                    // This node (main out) is connected to the 1st item of this default list (idx 0) by default.
                    [Some(NodeConnector::Single(0)), None],
                ),
            ]),
            ui_attributes: UiAttributes::default(),
            currently_selected_node_id: None,
        }
    }

    /// Displays the nodemap in the ui provided.
    pub fn display(&mut self, ui: &mut egui::Ui) {
        let desired_size = ui.available_size();
        ui.set_min_size(desired_size);
        ui.set_max_size(desired_size);

        let available_rect = ui.max_rect();

        // Display the background of the map
        self.display_background(ui, available_rect);

        // Allocate a response over the whole map which other responses will later lay on and steal the input.
        let bg = ui.allocate_rect(available_rect, Sense::click_and_drag());

        self.ui_attributes.offset += bg.drag_delta();

        if bg.hovered() {
            let scroll_delta = ui.input(|reader| reader.smooth_scroll_delta()).y;

            self.ui_attributes.scale =
                (self.ui_attributes.scale + scroll_delta * 0.01).clamp(0.1, 5.0);
        }

        // This will serve as the center point of out map.
        let reference_point = Pos2::new(
            available_rect.left() + available_rect.width() / 2.,
            available_rect.top() + available_rect.height() / 2.,
        );

        // Draw the nodes themselves
        for node in &mut self.nodes {
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

            let node_rect = node_rect.intersect(available_rect);

            // Draw the body of the node
            ui.painter().with_clip_rect(available_rect).rect_filled(
                node_rect,
                1.,
                Color32::DARK_GRAY,
            );

            // Create galley for sample label
            let galley = ui.fonts_mut(|f| {
                f.layout(
                    match node.node_type {
                        NodeType::In => "Input",
                        NodeType::Out => "Output",
                        NodeType::Plugin => "Plugin",
                    }
                    .to_string(),
                    egui::FontId::proportional(10.0 * self.ui_attributes.scale),
                    egui::Color32::WHITE,
                    node_rect.width(),
                )
            });

            // Calculate center of text
            let text_pos = center - galley.size() / 2.0;

            // Draw the name of the node
            ui.painter()
                .with_clip_rect(available_rect)
                .galley(text_pos, galley, Color32::WHITE);

            let node_response = ui.allocate_rect(node_rect, Sense::click_and_drag());

            if node_response.dragged() {
                // drag_delta() is in screen pixels; divide by scale to convert
                // that screen-space movement into the node's local coordinate space.
                node.position += node_response.drag_delta() / self.ui_attributes.scale;
            }

            // Draw connectors on each end (or either end) based on the node's type
            match node.node_type {
                // Samples coming in, left side of the screen.
                NodeType::In => {
                    // Fetch the connector's rect
                    let connector_pos = calculate_connector_pos(
                        node_rect,
                        Right,
                        node.node_connection[1].clone().unwrap(),
                    );
                    let node_rect = egui::Rect::from_center_size(connector_pos, vec2(10., 10.));

                    // Create a menu for the node if clicked
                    egui::Popup::menu(&node_response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                        ui.label(RichText::from("This is the main `In` node. This is the starting point of the samples' pipleline in the effects chain."))
                    });

                    // Paint connector
                    ui.painter().with_clip_rect(available_rect).rect_filled(
                        node_rect,
                        0.,
                        Color32::RED,
                    );
                }
                // Samples going out, right side of the screen. (normally)
                NodeType::Out => {
                    // Fetch the connector's rect
                    let connector_pos = calculate_connector_pos(
                        node_rect,
                        Left,
                        node.node_connection[0].clone().unwrap(),
                    );
                    let node_rect = egui::Rect::from_center_size(connector_pos, vec2(10., 10.));

                    // Create a menu for the node if clicked
                    egui::Popup::menu(&node_response).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                        ui.label(RichText::from("This is the main `Out` node. This is the end point of the samples' pipleline in the effects chain. The information that enters this node gets sent to the mixer."))
                    });

                    // Paint connector
                    ui.painter().with_clip_rect(available_rect).rect_filled(
                        node_rect,
                        0.,
                        Color32::BLUE,
                    );
                }
                NodeType::Plugin => {}
            };
        }
    }

    fn display_background(&self, ui: &mut egui::Ui, available_rect: egui::Rect) {
        // Display a black background in the available ui
        ui.painter().rect_filled(available_rect, 5., Color32::BLACK);

        // Display dots in the background with the given scaling every 15px
        let spacing = 15.0 * self.ui_attributes.scale;

        // Get maximum y coordinate
        let max_y = available_rect.bottom();
        // Get maximum x coordinate
        let max_x = available_rect.right();

        let start_x =
            available_rect.left() + self.ui_attributes.offset.x.rem_euclid(spacing) - spacing;
        let start_y =
            available_rect.top() + self.ui_attributes.offset.y.rem_euclid(spacing) - spacing;

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
}

fn calculate_connector_pos(
    node_rect: egui::Rect,
    side: ConnectorSide,
    conn_type: NodeConnector,
) -> Pos2 {
    if side == ConnectorSide::Right {
        Pos2::new(node_rect.right(), node_rect.top() + node_rect.height() / 2.)
    } else {
        Pos2::new(node_rect.left(), node_rect.top() + node_rect.height() / 2.)
    }
}
