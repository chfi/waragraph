use std::path::PathBuf;
use std::sync::Arc;

use crossbeam::atomic::AtomicCell;
use egui::{mutex::Mutex, Context, Id};
use egui_file::FileDialog;
use tokio::{
    sync::oneshot::{self, error::TryRecvError},
    task::JoinHandle,
};

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
        let id = egui::Id::new("Settings_Workspace");

        let state =
            WorkspaceWidgetState::load(ui.ctx(), id).unwrap_or_default();

        let mut file_picker_open = false;

        {
            let ch = state.tsv_path_recv.take();

            if let Some(mut ch) = ch {
                match ch.try_recv() {
                    Ok(path) => {
                        self.tsv_path = Some(path);
                    }
                    Err(e) => {
                        if matches!(e, TryRecvError::Empty) {
                            file_picker_open = true;
                            state.tsv_path_recv.store(Some(ch));
                        }
                    }
                }
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
                let mut files =
                    egui_file::FileDialog::open_file(self.tsv_path.clone())
                        .filter(Box::new(|p: &std::path::Path| {
                            if let Some(ext) =
                                p.extension().map(|e| e.to_ascii_lowercase())
                            {
                                ext == "tsv"
                            } else {
                                false
                            }
                        }));
                files.open();

                let recv = settings_ctx.with_file_dialog_oneshot(id, files);
                state.tsv_path_recv.store(Some(recv));
            }
        });

        state.store(ui.ctx(), id);

        SettingsUiResponse {
            response: resp.response,
        }
    }
}

#[derive(Default, Clone)]
pub struct WorkspaceWidgetState {
    // tsv_path_future: Arc<AtomicCell<Option<JoinHandle<Option<PathBuf>>>>>,
    tsv_path_recv: Arc<AtomicCell<Option<oneshot::Receiver<PathBuf>>>>,
}

impl WorkspaceWidgetState {
    pub fn load(ctx: &Context, id: Id) -> Option<Self> {
        ctx.data().get_temp(id)
    }

    pub fn store(self, ctx: &Context, id: Id) {
        ctx.data().insert_temp(id, self);
    }
}
