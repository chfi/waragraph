use std::path::PathBuf;

pub struct Workspace {
    pub(super) gfa_path: PathBuf,
    pub(super) tsv_path: Option<PathBuf>,
}

impl Workspace {
    pub fn gfa_path(&self) -> &PathBuf {
        &self.gfa_path
    }

    pub fn tsv_path(&self) -> Option<&PathBuf> {
        self.tsv_path.as_ref()
    }
}

impl egui::Widget for &mut Workspace {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal_wrapped(|ui| {
            ui.label("GFA:");
            let mut gfa_buf =
                self.gfa_path.clone().to_string_lossy().to_string();
            ui.add_enabled(false, egui::TextEdit::singleline(&mut gfa_buf));

            ui.end_row();

            let enabled = self.tsv_path.is_none();
            let mut tsv_buf = self
                .tsv_path
                .as_ref()
                .map(|tsv| tsv.to_string_lossy().to_string())
                .unwrap_or_default();

            ui.label("Layout:");
            ui.add_enabled(enabled, egui::TextEdit::singleline(&mut tsv_buf));

            if ui.button("Choose").clicked() {
                let mut files =
                    egui_file::FileDialog::open_file(self.tsv_path.clone())
                        .filter(Box::new(|p: &std::path::Path| {
                            p.ends_with(".tsv")
                        }));

                files.show(ui.ctx());

                if let Some(path) = files.path() {
                    self.tsv_path = Some(path);
                }
            }
        })
        .response
    }
}
