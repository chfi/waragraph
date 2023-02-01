use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::Arc,
};

use egui::mutex::Mutex;
use tokio::sync::{mpsc, oneshot, RwLock};

use super::AppMsg;

// pub struct AppSettings {
// }

// pub struct SettingsTab {
//     //
// }

struct SettingsHandler {
    name: String,
    widget: Arc<RwLock<dyn SettingsWidget + 'static>>,
}

pub struct SettingsWindowTab {
    name: String,
    handlers: Vec<SettingsHandler>,
}

impl SettingsWindowTab {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            handlers: Vec::new(),
        }
    }
}

pub struct SettingsWindow {
    // handlers: Vec<SettingsHandler>,
    tabs: BTreeMap<String, SettingsWindowTab>,

    active_tab: Option<String>,

    ctx: SettingsUiContext,
}

impl SettingsWindow {
    pub fn new(
        tokio_handle: tokio::runtime::Handle,
        app_msg_send: mpsc::Sender<AppMsg>,
    ) -> Self {
        Self {
            tabs: BTreeMap::default(),
            active_tab: None,
            ctx: SettingsUiContext::new(tokio_handle, app_msg_send),
        }
    }

    pub fn register_widget(
        &mut self,
        tab_name: &str,
        name: &str,
        widget: Arc<RwLock<dyn SettingsWidget + 'static>>,
    ) {
        let tab = self
            .tabs
            .entry(tab_name.into())
            .or_insert_with(|| SettingsWindowTab::new(name));

        let h = SettingsHandler {
            name: name.to_string(),
            widget,
        };

        tab.handlers.push(h);
    }

    fn validate_active_tab(&mut self) {
        let need_fix = self
            .active_tab
            .as_ref()
            .map(|tab_name| !self.tabs.contains_key(tab_name))
            .unwrap_or(true);

        if need_fix {
            if let Some(tab_name) = self.tabs.keys().next() {
                self.active_tab = Some(tab_name.clone());
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.process_file_dialogs(ctx);

        self.validate_active_tab();

        egui::Window::new("Settings").show(ctx, |ui| {
            ui.set_min_size(egui::vec2(500.0, 400.0));
            egui::Grid::new("SettingsWindowGrid").num_columns(2).show(
                ui,
                |ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(120.0);
                        for tab_name in self.tabs.keys() {
                            let enabled =
                                Some(tab_name) == self.active_tab.as_ref();
                            let button = ui.add_enabled(
                                enabled,
                                egui::Button::new(tab_name),
                            );

                            if button.clicked() {
                                self.active_tab = Some(tab_name.to_string());
                            }
                        }
                    });

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .min_scrolled_height(400.0)
                        .show(ui, |ui| {
                            if let Some(active_tab) = self.active_tab.as_ref() {
                                if let Some(tab) = self.tabs.get_mut(active_tab)
                                {
                                    for h in tab.handlers.iter_mut() {
                                        let name = &h.name;
                                        let widget = &mut h.widget;

                                        ui.collapsing(name, |ui| {
                                            let mut lock =
                                                widget.blocking_write();
                                            let _resp =
                                                lock.show(ui, &self.ctx);
                                        });
                                    }
                                }
                            }
                        });

                    ui.end_row();
                },
            );
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
pub struct SettingsUiContext {
    pub tokio_handle: tokio::runtime::Handle,
    pub app_msg_send: mpsc::Sender<AppMsg>,

    file_dialogs: Mutex<HashMap<egui::Id, FileDialogState>>,
}

impl SettingsUiContext {
    pub fn send_app_msg_task(&self, app_msg: AppMsg) {
        let send = self.app_msg_send.clone();
        self.tokio_handle
            .spawn(async move { send.send(app_msg).await });
    }

    pub fn new(
        tokio_handle: tokio::runtime::Handle,
        app_msg_send: mpsc::Sender<AppMsg>,
    ) -> Self {
        Self {
            tokio_handle,
            app_msg_send,

            file_dialogs: Default::default(),
        }
    }

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
