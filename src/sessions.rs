use crate::chat::Chat;
use eframe::egui;
use ollama_rs::Ollama;
use parking_lot::RwLock;
use std::sync::Arc;
use tts::Tts;

#[derive(Default, PartialEq)]
enum SessionTab {
    #[default]
    Chats,
    Model,
}

pub type SharedTts = Option<Arc<RwLock<Tts>>>;

pub struct Sessions {
    tab: SessionTab,
    chats: Vec<Chat>,
    selected_chat: Option<usize>,
    is_speaking: bool,
    tts: SharedTts,
}

impl Default for Sessions {
    fn default() -> Self {
        Self {
            tab: SessionTab::Chats,
            chats: vec![Chat::default()],
            selected_chat: Some(0),
            is_speaking: false,
            tts: Tts::default()
                .map_err(|e| log::error!("failed to initialize TTS: {e}"))
                .map(|tts| Arc::new(RwLock::new(tts)))
                .ok(),
        }
    }
}

impl Sessions {
    pub fn show(&mut self, ctx: &egui::Context, ollama: Arc<Ollama>) {
        // check if tts stopped speaking
        let prev_is_speaking = self.is_speaking;
        self.is_speaking = if let Some(tts) = &self.tts {
            tts.read().is_speaking().unwrap_or(false)
        } else {
            false
        };

        // if speaking, continuously check if stopped
        if self.is_speaking {
            ctx.request_repaint();
        }

        let avail_width = ctx.available_rect().width();
        egui::SidePanel::left("sessions_panel")
            .resizable(true)
            .max_width(avail_width * 0.5)
            .show(ctx, |ui| {
                self.show_left_panel(ui);
                ui.allocate_space(ui.available_size());
            });

        let tts = self.tts.clone();
        let is_speaking = self.is_speaking;
        if let Some(chat) = self.get_selected_chat() {
            chat.show(
                ctx,
                ollama.clone(),
                tts,
                prev_is_speaking && !is_speaking, // stopped_talking
            );
        }
    }

    fn show_left_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(ui.style().spacing.window_margin.top);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, SessionTab::Chats, "Chats");
            ui.selectable_value(&mut self.tab, SessionTab::Model, "Model");
        });

        ui.add_space(8.0);

        match self.tab {
            SessionTab::Chats => {
                self.show_chats(ui);
            }
            SessionTab::Model => {
                ui.label("Model");
            }
        }
    }

    #[inline]
    fn get_selected_chat(&mut self) -> Option<&mut Chat> {
        self.chats.get_mut(self.selected_chat?)
    }

    fn show_chats(&mut self, ui: &mut egui::Ui) {
        if ui.button("➕ New Chat").clicked() {
            self.chats.push(Chat::default());
        }
        for (i, chat) in self.chats.iter().enumerate() {
            if ui
                .button(if chat.summary.is_empty() {
                    "Empty chat"
                } else {
                    &chat.summary
                })
                .clicked()
            {
                self.selected_chat = Some(i);
            }
        }
    }
}
