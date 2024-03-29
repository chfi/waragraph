use waragraph_core::graph::{Bp, Node, PathId};

use crate::app::SharedState;

use super::view::View1D;

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
    pub fn apply(self, shared: &SharedState, view: &mut View1D) {
        match self {
            ViewCmd::GotoNode { node } => {
                let range = shared.graph.node_pangenome_range(node);
                view.try_center(range);
            }
            ViewCmd::GotoRange { path, range } => {
                let range = if let Some(path) = path {
                    // TODO: this just reduces to the pangenome
                    // interval containing the nodes in the path
                    // range; it doesn't try to find the correct
                    // position on the bp-level

                    let steps = shared.graph.path_step_range_iter(path, range);

                    let node_bounds = steps
                        .map(|steps| {
                            steps.fold(
                                (u32::MAX, u32::MIN),
                                |(min, max), (_, step)| {
                                    let min = min.min(step.node().ix() as u32);
                                    let max = max.max(step.node().ix() as u32);
                                    (min, max)
                                },
                            )
                        })
                        .filter(|&(min, max)| {
                            min != u32::MAX && max != u32::MIN
                        })
                        .map(|(min, max)| (Node::from(min), Node::from(max)));

                    if let Some((min_n, max_n)) = node_bounds {
                        let (left, _) = shared.graph.node_offset_length(min_n);
                        let (r_off, r_len) =
                            shared.graph.node_offset_length(max_n);
                        let right = Bp(r_off.0 + r_len.0);

                        left..right
                    } else {
                        return;
                    }
                } else {
                    // the pangenome range interval is exact
                    range
                };

                view.try_center(range);
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
            let node = parse_node(&self.node_id_text);

            if let Some(range) =
                node.map(|n| self.shared.graph.node_pangenome_range(n))
            {
                let _ = self
                    .msg_tx
                    .send(Msg::View(ViewCmd::GotoRange { path: None, range }));
            }
        }

        if goto_pos {
            if let Some((path_name, range)) = parse_pos_range(&self.pos_text) {
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

pub fn parse_node(text: &str) -> Option<Node> {
    text.parse::<u32>().map(Node::from).ok()
}

pub fn parse_pos_range(
    text: &str,
) -> Option<(Option<&str>, std::ops::Range<Bp>)> {
    fn parse_range(text: &str) -> Option<std::ops::Range<Bp>> {
        if let Some((from, to)) = text.split_once("-") {
            let from = from.parse::<u64>().ok()?;
            let to = to.parse::<u64>().ok()?;
            Some(Bp(from)..Bp(to))
        } else {
            let pos = text.parse::<u64>().ok()?;
            Some(Bp(pos)..Bp(pos + 1))
        }
    }

    if let Some((path_name, range_text)) = text.rsplit_once(":") {
        let range = parse_range(range_text)?;
        Some((Some(path_name), range))
    } else {
        let range = parse_range(text)?;
        Some((None, range))
    }
}
