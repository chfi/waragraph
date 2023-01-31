use std::path::PathBuf;
use std::sync::Arc;

use egui::{mutex::Mutex, Context, Id};
use egui_file::FileDialog;

use super::settings_menu::{
    SettingsUiContext, SettingsUiResponse, SettingsWidget,
};

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

impl SettingsWidget for Workspace {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings_ctx: &SettingsUiContext,
    ) -> SettingsUiResponse {
        todo!();
    }
}

impl egui::Widget for &mut Workspace {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let id = egui::Id::new("Settings_Workspace");

        let mut state =
            WorkspaceWidgetState::load(ui.ctx(), id).unwrap_or_default();

        let file_picker_open = {
            let mut lock = state.open_file_dialog.lock();
            if let Some(dialog) = lock.as_mut() {
                if dialog.show(ui.ctx()).selected() {
                    if let Some(path) = dialog.path() {
                        self.tsv_path = Some(path);
                    }
                }

                true
            } else {
                false
            }
        };

        let resp = ui.horizontal_wrapped(|ui| {
            ui.label("GFA:");
            let mut gfa_buf =
                self.gfa_path.clone().to_string_lossy().to_string();
            ui.add_enabled(false, egui::TextEdit::singleline(&mut gfa_buf));

            ui.end_row();

            let enabled = self.tsv_path.is_none() && !file_picker_open;
            let mut tsv_buf = self
                .tsv_path
                .as_ref()
                .map(|tsv| tsv.to_string_lossy().to_string())
                .unwrap_or_default();

            ui.label("Layout:");
            ui.add_enabled(enabled, egui::TextEdit::singleline(&mut tsv_buf));

            if ui.button("Choose").clicked() {
                let files =
                    egui_file::FileDialog::open_file(self.tsv_path.clone())
                        .filter(Box::new(|p: &std::path::Path| {
                            p.ends_with(".tsv")
                        }));

                let mut lock = state.open_file_dialog.lock();
                *lock = Some(files);
            }
        });

        state.store(ui.ctx(), id);

        resp.response
    }
}

#[derive(Default, Clone)]
pub struct WorkspaceWidgetState {
    open_file_dialog: Arc<Mutex<Option<FileDialog>>>,
}

impl WorkspaceWidgetState {
    pub fn load(ctx: &Context, id: Id) -> Option<Self> {
        ctx.data().get_temp(id)
    }

    pub fn store(self, ctx: &Context, id: Id) {
        ctx.data().insert_temp(id, self);
    }
}
