use std::collections::LinkedList;

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2, vec2};

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
    connection_order: LinkedList<Node>,
    ui_attributes: UiAttributes,
}

impl NodeMap {
    /// Constructs an empty nodemap.
    pub fn new() -> Self {
        Self {
            connection_order: LinkedList::from([
                Node::new(NodeType::In, Pos2::new(-300.0, 0.), vec2(80., 25.)),
                Node::new(NodeType::Out, Pos2::new(300., 0.), vec2(80., 25.)),
            ]),
            ui_attributes: UiAttributes::default(),
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

        // The last node's connector's location - the next node will automatically connect to that and set its out port to the next nodes in.
        let mut last_connector_pos: Option<Pos2> = None;

        // Draw the nodes themselves
        for node in &mut self.connection_order {
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
                    }
                    .to_string(),
                    egui::FontId::proportional(10.0 * self.ui_attributes.scale),
                    egui::Color32::WHITE,
                    node_rect.width(),
                )
            });

            // Draw the name of the node
            ui.painter()
                .with_clip_rect(available_rect)
                .galley(center, galley, Color32::WHITE);

            let node_response = ui.allocate_rect(node_rect, Sense::click_and_drag());

            if node_response.dragged() {
                // drag_delta() is in screen pixels; divide by scale to convert
                // that screen-space movement into the node's local coordinate space.
                node.position += node_response.drag_delta() / self.ui_attributes.scale;
            }

            // Draw connectors on each end (or either end) based on the node's type
            let current_connector_pos = match node.node_type {
                // Samples coming in, left side of the screen.
                NodeType::In => {
                    // Fetch the connector's rect
                    let connector_pos =
                        Pos2::new(node_rect.right(), node_rect.top() + node_rect.height() / 2.);
                    // Paint connector
                    ui.painter().with_clip_rect(available_rect).rect_filled(
                        egui::Rect::from_center_size(connector_pos, vec2(10., 10.)),
                        0.,
                        Color32::RED,
                    );

                    connector_pos
                }
                // Samples going out, right side of the screen. (normally)
                NodeType::Out => {
                    // Fetch the connector's rect
                    let connector_pos =
                        Pos2::new(node_rect.left(), node_rect.top() + node_rect.height() / 2.);
                    // Paint connector
                    ui.painter().with_clip_rect(available_rect).rect_filled(
                        egui::Rect::from_center_size(connector_pos, vec2(10., 10.)),
                        0.,
                        Color32::BLUE,
                    );

                    connector_pos
                }
            };

            // Connect the last and the current connector's rect if there was a node already
            if let Some(last_connector_pos) = last_connector_pos.clone() {
                ui.painter().with_clip_rect(available_rect).line(
                    vec![last_connector_pos, current_connector_pos],
                    Stroke::new(2.0_f32, Color32::WHITE),
                );
            }

            // Update last connector position
            last_connector_pos = Some(current_connector_pos);
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

pub enum NodeType {
    /// Main sample in.
    /// This is where the (resampled) original samples flow into the map.
    In,

    /// Main sample out.
    /// This is where the final samples flow out of the map after all the effects (if any) have been applied to them.
    Out,
}

pub struct Node {
    node_type: NodeType,

    position: Pos2,
    size: Vec2,
}

impl Node {
    pub fn new(node_type: NodeType, position: Pos2, size: Vec2) -> Self {
        Self {
            node_type,
            position,
            size,
        }
    }
}
