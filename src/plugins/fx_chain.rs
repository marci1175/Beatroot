use std::collections::LinkedList;

use egui::{Color32, Pos2};

pub struct UiAttributes {
    scale: f32,
}

impl Default for UiAttributes {
    fn default() -> Self {
        Self { scale: 1.0 }
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
            connection_order: LinkedList::from([Node::new(NodeType::In), Node::new(NodeType::Out)]),
            ui_attributes: UiAttributes::default(),
        }
    }

    /// Displays the nodemap in the ui provided.
    pub fn display(&self, ui: &mut egui::Ui) {
        let available_rect = ui.min_rect();

        self.display_background(ui, available_rect);
    }

    fn display_background(&self, ui: &mut egui::Ui, available_rect: egui::Rect) {
        // Display a black blackground in the available ui
        ui.painter().rect_filled(available_rect, 5., Color32::BLACK);

        // Display dots in the background with the given scaling every 100px
        let mut y_coord = available_rect.top();

        // Get maximum y coordinate
        let max_y = available_rect.bottom();
        // Get maximum x coordinate
        let max_x = available_rect.right();

        // Iter over all the y coordinates
        while y_coord < max_y {
            // Iterate over the x coordinates
            let mut x_coord = available_rect.left();

            while x_coord < max_x {
                // Draw dots to make it look more pleasing to the eye
                ui.painter()
                    .circle_filled(Pos2::new(x_coord, y_coord), 2., Color32::GRAY);

                // Increment x coordinate
                x_coord += 100.0 * self.ui_attributes.scale;
            }

            // Increment y coordinate
            y_coord += 100.0 * self.ui_attributes.scale;
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
}

impl Node {
    pub fn new(node_type: NodeType) -> Self {
        Self { node_type }
    }
}
