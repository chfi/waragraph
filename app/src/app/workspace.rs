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
        let enabled = self.tsv_path.is_none();

        let files = egui_file::FileDialog::open_file(self.tsv_path.clone())
            .filter(Box::new(|p: &std::path::Path| p.ends_with(".tsv")));

        // let mut gfa_buf = self.gfa_path.clone().to_string_lossy().to_string();
        // ui.add_enabled(false, egui::TextEdit::singleline(&mut
        todo!();
    }
}
