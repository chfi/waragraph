use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

use egui::mutex::Mutex;
use tokio::sync::{oneshot, RwLock};

// pub struct AppSettings {
// }

// pub struct SettingsTab {
//     //
// }

struct SettingsHandler {
    name: String,
    // show: Box<dyn Fn(&mut egui::Ui) + Send + Sync + 'static>,
    // validate: Option<Box<dyn Fn() -> bool + Send + Sync + 'static>;
    widget: Arc<RwLock<dyn SettingsWidget + 'static>>,
}

#[derive(Default)]
pub struct SettingsWindow {
    handlers: Vec<SettingsHandler>,

    ctx: SettingsUiContext,
}

impl SettingsWindow {
    pub fn register_widget(
        &mut self,
        name: &str,
        // widget: impl SettingsWidget + 'static,
        widget: Arc<RwLock<dyn SettingsWidget + 'static>>,
    ) {
        let h = SettingsHandler {
            name: name.to_string(),
            widget,
        };

        self.handlers.push(h);
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.process_file_dialogs(ctx);

        egui::Window::new("Settings").show(ctx, |ui| {
            for h in self.handlers.iter_mut() {
                let name = &h.name;
                let widget = &mut h.widget;

                ui.collapsing(name, |ui| {
                    let mut lock = widget.blocking_write();
                    let _resp = lock.show(ui, &self.ctx);
                });
            }
        });
    }
}

impl SettingsWindow {
    fn process_file_dialogs(&mut self, ctx: &egui::Context) {
        let mut lock = self.ctx.file_dialogs.lock();

        let mut done = Vec::new();

        for (id, dialog) in lock.iter_mut() {
            if dialog.dialog.show(ctx).selected() {
                done.push(*id);
            }
        }

        for id in done {
            if let Some(dialog) = lock.remove(&id) {
                let path = dialog.dialog.path();
                (dialog.callback)(path);
            }
        }
    }
}

struct FileDialogState {
    dialog: egui_file::FileDialog,
    callback: Box<dyn FnOnce(Option<PathBuf>) + Send + Sync + 'static>,
}

// provides the interface for opening file dialogs etc. from settings widgets
#[derive(Default)]
pub struct SettingsUiContext {
    file_dialogs: Mutex<HashMap<egui::Id, FileDialogState>>,
}

impl SettingsUiContext {
    pub fn with_file_dialog_oneshot(
        &self,
        id: egui::Id,
        dialog: egui_file::FileDialog,
    ) -> oneshot::Receiver<PathBuf> {
        let (send, recv) = oneshot::channel::<PathBuf>();

        let f = move |path| {
            if let Some(path) = path {
                let _ = send.send(path);
            }
        };

        let state = FileDialogState {
            dialog,
            callback: Box::new(f),
        };

        let mut lock = self.file_dialogs.lock();
        lock.insert(id, state);

        recv
    }
}

pub struct SettingsUiResponse {
    pub response: egui::Response,
}

pub trait SettingsWidget {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings_ctx: &SettingsUiContext,
    ) -> SettingsUiResponse;
}
