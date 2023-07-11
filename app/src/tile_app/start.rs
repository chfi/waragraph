use egui_file::FileDialog;

use std::path::PathBuf;

#[derive(Default)]
pub struct StartPage {
    gfa_path: Option<PathBuf>,
    tsv_path: Option<PathBuf>,

    file_dialog: Option<(DialogTarget, FileDialog)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DialogTarget {
    GFA,
    TSV,
}

impl StartPage {
    fn file_dialog_row(
        ui: &mut egui::Ui,
        path: &mut Option<PathBuf>,
        tgt: DialogTarget,
        file_dialog: &mut Option<(DialogTarget, FileDialog)>,
        tgt_ext: &str,
        button_text: &str,
    ) {
        if matches!(
            file_dialog.as_ref().map(|(_, d)| d.state()),
            Some(egui_file::State::Closed | egui_file::State::Cancelled)
        ) {
            file_dialog.take();
        }

        let button = ui
            .add_enabled(file_dialog.is_none(), egui::Button::new(button_text));

        if button.clicked() {
            let tgt_ext = tgt_ext.to_string();
            let mut dialog = FileDialog::open_file(path.clone())
                // .
                .filter(Box::new(move |p: &std::path::Path| {
                    p.extension()
                        .map(|e| e.to_ascii_lowercase())
                        .is_some_and(|ext| ext == tgt_ext.as_str())
                }));

            dialog.open();

            *file_dialog = Some((tgt, dialog));
        }

        let mut buf = path
            .as_ref()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        ui.add_enabled(false, egui::TextEdit::singleline(&mut buf));
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
    ) -> Option<super::ResourceLoadState> {
        let mut result = None;

        if let Some((tgt, dialog)) = self.file_dialog.as_mut() {
            dialog.show(ui.ctx());

            if dialog.selected() {
                let path = dialog.path();
                match tgt {
                    DialogTarget::GFA => self.gfa_path = path,
                    DialogTarget::TSV => self.tsv_path = path,
                }
                self.file_dialog = None;
            }
        }

        ui.vertical_centered_justified(|ui| {
            Self::file_dialog_row(
                ui,
                &mut self.gfa_path,
                DialogTarget::GFA,
                &mut self.file_dialog,
                "gfa",
                "Open graph GFA",
            );
            ui.end_row();

            Self::file_dialog_row(
                ui,
                &mut self.tsv_path,
                DialogTarget::TSV,
                &mut self.file_dialog,
                "tsv",
                "Open graph layout TSV",
            );

            ui.end_row();

            if ui.button("Load").clicked() {
                result = Some(super::ResourceLoadState {
                    gfa_path: self.gfa_path.clone(),
                    tsv_path: self.tsv_path.clone(),
                    graph: None,
                    node_positions: None,
                });
            }
            ui.end_row();
        });

        result
    }
}
