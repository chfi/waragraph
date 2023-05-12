use waragraph_core::graph::{Bp, Node, PathId};

use crate::app::SharedState;

use super::{layout::NodePositions, view::View2D};

pub enum Msg {
    View(ViewCmd),
}

struct ViewMsgParams {
    zoom: bool,
}

pub enum ViewCmd {
    GotoNode {
        node: Node,
    },
    GotoRange {
        path: Option<PathId>,
        range: std::ops::Range<Bp>,
    },
}

impl ViewCmd {
    pub fn apply(
        self,
        shared: &SharedState,
        node_layout: &NodePositions,
        view: &mut View2D,
    ) {
        match self {
            ViewCmd::GotoNode { node } => {
                // TODO improve; make sure the scale is correct (i.e.
                // the node fits on the screen properly)
                let (p0, p1) = node_layout.node_pos(node);
                let mid = p0 + (p1 - p0) * 0.5;
                view.center = mid;
            }
            ViewCmd::GotoRange { path, range } => {
                use ultraviolet::Vec2;

                let bounds = if let Some(path) = path {
                    let steps = shared.graph.path_step_range_iter(path, range);

                    if steps.is_none() {
                        return;
                    }

                    let steps = steps
                        .unwrap()
                        .map(|(_, step)| node_layout.node_pos(step.node()));

                    let bounds = steps.fold(
                        (
                            Vec2::broadcast(f32::INFINITY.into()),
                            Vec2::broadcast(f32::NEG_INFINITY.into()),
                        ),
                        |(min, max), (p0, p1)| {
                            let min =
                                min.min_by_component(p0).min_by_component(p1);
                            let max =
                                max.max_by_component(p0).max_by_component(p1);
                            (min, max)
                        },
                    );

                    bounds
                } else {
                    let node0 = shared.graph.node_at_pangenome_pos(range.start);
                    let node1 = shared.graph.node_at_pangenome_pos(range.end);

                    let bounds = node0.zip(node1).and_then(|(n0, n1)| {
                        let (a0, a1) = node_layout.node_pos(n0);
                        let (b0, b1) = node_layout.node_pos(n1);

                        let points = [a0, a1, b0, b1];

                        let bounds = points.into_iter().fold(
                            (
                                Vec2::broadcast(f32::INFINITY.into()),
                                Vec2::broadcast(f32::NEG_INFINITY.into()),
                            ),
                            |(min, max), p| {
                                let min = min.min_by_component(p);
                                let max = max.max_by_component(p);
                                (min, max)
                            },
                        );

                        if bounds.0.component_min().is_infinite()
                            || bounds.1.component_max().is_infinite()
                        {
                            None
                        } else {
                            Some(bounds)
                        }
                    });

                    if let Some(bounds) = bounds {
                        bounds
                    } else {
                        return;
                    }
                };

                // TODO set scale as well

                let (p0, p1) = bounds;

                let mid = p0 + (p1 - p0) * 0.5;
                view.center = mid;
            }
        }
    }
}

pub struct ViewControlWidget {
    shared: SharedState,
    msg_tx: crossbeam::channel::Sender<Msg>,

    node_id_text: String,
    pos_text: String,
}

impl ViewControlWidget {
    pub fn new(
        shared: &SharedState,
        msg_tx: crossbeam::channel::Sender<Msg>,
    ) -> Self {
        Self {
            shared: shared.clone(),
            msg_tx,

            node_id_text: String::new(),
            pos_text: String::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.label("Node ID");
        let node_id_entry = ui.add_sized(
            [ui.available_size().x, 0f32],
            egui::TextEdit::singleline(&mut self.node_id_text),
        );
        let goto_node_b = ui.button("Go to node");

        let goto_node = goto_node_b.clicked()
            || (node_id_entry.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter)));

        ui.label("Position");
        let pos_entry = ui.add_sized(
            [ui.available_size().x, 0f32],
            egui::TextEdit::singleline(&mut self.pos_text),
        );

        let goto_pos_b = ui.button("Go to position");

        let goto_pos = goto_pos_b.clicked()
            || (pos_entry.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter)));

        if goto_node {
            let node =
                crate::viewer_1d::control::parse_node(&self.node_id_text);

            if let Some(node) = node {
                let _ = self.msg_tx.send(Msg::View(ViewCmd::GotoNode { node }));
            }
        }

        if goto_pos {
            if let Some((path_name, range)) =
                crate::viewer_1d::control::parse_pos_range(&self.pos_text)
            {
                let path = path_name
                    .and_then(|name| {
                        self.shared.graph.path_names.get_by_right(name)
                    })
                    .copied();

                let _ = self
                    .msg_tx
                    .send(Msg::View(ViewCmd::GotoRange { path, range }));
            }
        }
    }
}
