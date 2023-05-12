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
    // GotoPos {
    //     path: Option<PathId>,
    //     pos: Bp,
    // },
    GotoRange {
        path: Option<PathId>,
        range: std::ops::Range<Bp>,
    },
}

impl ViewCmd {
    pub fn apply(self, shared: &SharedState, view: &mut View1D) {
        match self {
            ViewCmd::GotoRange { path, range } => {
                let range = if let Some(path) = path {
                    todo!();
                } else {
                    range
                };

                view.try_center(range);
            }
        }
    }
}

// fn goto_pos(shared: &SharedState, path: Option<PathId>,

pub struct ViewControlWidget {
    shared: SharedState,
    msg_tx: crossbeam::channel::Sender<Msg>,

    node_id_text: String,
    pos_text: String,
    // node_id: Option<
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
        let node_id_entry = ui.text_edit_singleline(&mut self.node_id_text);
        let goto_node_b = ui.button("Go to node");

        let goto_node = goto_node_b.clicked()
            || (node_id_entry.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter)));

        ui.label("Position");
        let pos_entry = ui.text_edit_singleline(&mut self.pos_text);
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

fn parse_node(text: &str) -> Option<Node> {
    text.parse::<u32>().map(Node::from).ok()
}

fn parse_pos_range(text: &str) -> Option<(Option<&str>, std::ops::Range<Bp>)> {
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
