use eframe::{
    egui::{self, Color32},
    emath::Numeric,
};
use ollama_rs::{
    generation::options::GenerationOptions,
    models::{LocalModel, ModelInfo},
};

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SelectedModel {
    pub name: String,
    #[serde(default)]
    pub short_name: String,
    #[serde(default)]
    modified_ago: String,
    modified_at: String,
    size: u64,
}

/// Convert a model name into a short name.
///
/// # Example
///
/// - nous-hermes2:latest -> Nous
/// - gemma:latest -> Gemma
/// - starling-lm:7b-beta-q5_K_M -> Starling
fn make_short_name(name: &str) -> String {
    let mut c = name.chars().take_while(|c| c.is_alphanumeric());
    match c.next() {
        None => "Llama".to_string(),
        Some(f) => f.to_uppercase().collect::<String>() + c.collect::<String>().as_str(),
    }
}

impl From<LocalModel> for SelectedModel {
    fn from(model: LocalModel) -> Self {
        let ago = chrono::DateTime::parse_from_rfc3339(&model.modified_at)
            .map(|time| timeago::Formatter::new().convert_chrono(time, chrono::Utc::now()))
            .unwrap_or_else(|e| e.to_string());
        Self {
            short_name: make_short_name(&model.name),
            name: model.name,
            modified_ago: ago,
            modified_at: model.modified_at,
            size: model.size,
        }
    }
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct ModelPicker {
    pub selected: SelectedModel,
    pub info: Option<ModelInfo>,
    settings: ModelSettings,
}

pub enum RequestInfoType<'a> {
    Models,
    ModelInfo(&'a str),
}

impl ModelPicker {
    pub fn show<R>(&mut self, ui: &mut egui::Ui, models: Option<&[LocalModel]>, mut request_info: R)
    where
        R: FnMut(RequestInfoType),
    {
        if let Some(models) = models {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_source("model_selector_combobox")
                    .selected_text(&self.selected.name)
                    .show_ui(ui, |ui| {
                        for model in models {
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(self.selected.name == model.name, &model.name)
                                    .clicked()
                                {
                                    self.selected = model.clone().into();
                                    self.info = None;
                                }
                                // TODO: make this stick to the right
                                ui.add_enabled(
                                    false,
                                    egui::Label::new(format!("{}", bytesize::ByteSize(model.size))),
                                );
                            });
                        }
                        if models.is_empty() {
                            ui.label("No models found, is the server running?");
                        }
                    });
                if ui
                    .add(egui::Button::new("⟳").small().fill(Color32::TRANSPARENT))
                    .on_hover_text("Refresh model list")
                    .clicked()
                {
                    request_info(RequestInfoType::Models);
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label("Loading model list…");
            });
        }

        if !self.has_selection() {
            return;
        }

        ui.collapsing("Settings", |ui| {
            self.settings.show(ui);
        });

        ui.separator();

        egui::Grid::new("selected_model_info_grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Size");
                ui.label(format!("{}", bytesize::ByteSize(self.selected.size)))
                    .on_hover_text(format!("{} bytes", self.selected.size));
                ui.end_row();

                ui.label("Modified");
                ui.add(egui::Label::new(&self.selected.modified_ago).truncate(true))
                    .on_hover_text(&self.selected.modified_at);
                ui.end_row();
            });

        if let Some(info) = &self.info {
            for (heading, mut text) in [
                ("License", info.license.as_str()),
                ("Modelfile", info.modelfile.as_str()),
                ("Parameters", info.parameters.as_str()),
                ("Template", info.template.as_str()),
            ] {
                if !text.is_empty() {
                    ui.collapsing(heading, |ui| {
                        ui.code_editor(&mut text);
                    });
                }
            }
        } else {
            request_info(RequestInfoType::ModelInfo(&self.selected.name));
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label("Loading model info…");
            });
        }
    }

    pub fn on_new_model_info(&mut self, name: &str, info: &ModelInfo) {
        if self.selected.name == name {
            self.info = Some(info.clone());
        }
    }

    pub fn select_best_model(&mut self, models: &[LocalModel]) {
        models
            .iter()
            .max_by_key(|m| m.size)
            .map(|m| self.selected = m.clone().into());
        if self.has_selection() {
            log::info!("subjectively selected best model: {}", self.selected.name);
        }
    }

    #[inline]
    pub fn has_selection(&self) -> bool {
        !self.selected.name.is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum Mirostat {
    Disabled,
    Mirostat,
    Mirostat2,
}

impl Mirostat {
    #[inline]
    const fn to_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    const fn name(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Mirostat => "Mirostat",
            Self::Mirostat2 => "Mirostat 2.0",
        }
    }
}

#[derive(Default, Clone, serde::Deserialize, serde::Serialize)]
struct ModelSettings {
    /// Enable Mirostat sampling for controlling perplexity. (default: 0, 0 = disabled, 1 = Mirostat, 2 = Mirostat 2.0)
    pub mirostat: Option<Mirostat>,
    /// Influences how quickly the algorithm responds to feedback from the generated text. A lower learning rate will result in slower adjustments, while a higher learning rate will make the algorithm more responsive. (Default: 0.1)
    pub mirostat_eta: Option<f32>,
    /// Controls the balance between coherence and diversity of the output. A lower value will result in more focused and coherent text. (Default: 5.0)
    pub mirostat_tau: Option<f32>,
    /// Sets the size of the context window used to generate the next token. (Default: 2048)
    pub num_ctx: Option<u32>,
    /// The number of GQA groups in the transformer layer. Required for some models, for example it is 8 for llama2:70b
    pub num_gqa: Option<u32>,
    /// The number of layers to send to the GPU(s). On macOS it defaults to 1 to enable metal support, 0 to disable.
    pub num_gpu: Option<u32>,
    /// Sets the number of threads to use during computation. By default, Ollama will detect this for optimal performance. It is recommended to set this value to the number of physical CPU cores your system has (as opposed to the logical number of cores).
    pub num_thread: Option<u32>,
    /// Sets how far back for the model to look back to prevent repetition. (Default: 64, 0 = disabled, -1 = num_ctx)
    pub repeat_last_n: Option<i32>,
    /// Sets how strongly to penalize repetitions. A higher value (e.g., 1.5) will penalize repetitions more strongly, while a lower value (e.g., 0.9) will be more lenient. (Default: 1.1)
    pub repeat_penalty: Option<f32>,
    /// The temperature of the model. Increasing the temperature will make the model answer more creatively. (Default: 0.8)
    pub temperature: Option<f32>,
    /// Sets the random number seed to use for generation. Setting this to a specific number will make the model generate the same text for the same prompt. (Default: 0)
    pub seed: Option<i32>,
    /// Sets the stop sequences to use. When this pattern is encountered the LLM will stop generating text and return. Multiple stop patterns may be set by specifying multiple separate `stop` parameters in a modelfile.
    pub stop: Option<Vec<String>>,
    /// Tail free sampling is used to reduce the impact of less probable tokens from the output. A higher value (e.g., 2.0) will reduce the impact more, while a value of 1.0 disables this setting. (default: 1)
    pub tfs_z: Option<f32>,
    /// Maximum number of tokens to predict when generating text. (Default: 128, -1 = infinite generation, -2 = fill context)
    pub num_predict: Option<i32>,
    /// Reduces the probability of generating nonsense. A higher value (e.g. 100) will give more diverse answers, while a lower value (e.g. 10) will be more conservative. (Default: 40)
    pub top_k: Option<u32>,
    /// Works together with top-k. A higher value (e.g., 0.95) will lead to more diverse text, while a lower value (e.g., 0.5) will generate more focused and conservative text. (Default: 0.9)
    pub top_p: Option<f32>,
}

impl From<ModelSettings> for GenerationOptions {
    fn from(value: ModelSettings) -> Self {
        let mut s = Self::default();
        if let Some(mirostat) = value.mirostat {
            s = s.mirostat(mirostat.to_u8());
        }
        if let Some(mirostat_eta) = value.mirostat_eta {
            s = s.mirostat_eta(mirostat_eta);
        }
        if let Some(mirostat_tau) = value.mirostat_tau {
            s = s.mirostat_tau(mirostat_tau);
        }
        if let Some(num_ctx) = value.num_ctx {
            s = s.num_ctx(num_ctx);
        }
        if let Some(num_gqa) = value.num_gqa {
            s = s.num_gqa(num_gqa);
        }
        if let Some(num_gpu) = value.num_gpu {
            s = s.num_gpu(num_gpu);
        }
        if let Some(num_thread) = value.num_thread {
            s = s.num_thread(num_thread);
        }
        if let Some(repeat_last_n) = value.repeat_last_n {
            s = s.repeat_last_n(repeat_last_n);
        }
        if let Some(repeat_penalty) = value.repeat_penalty {
            s = s.repeat_penalty(repeat_penalty);
        }
        if let Some(temperature) = value.temperature {
            s = s.temperature(temperature);
        }
        if let Some(seed) = value.seed {
            s = s.seed(seed);
        }
        if let Some(stop) = value.stop {
            s = s.stop(stop);
        }
        if let Some(tfs_z) = value.tfs_z {
            s = s.tfs_z(tfs_z);
        }
        if let Some(num_predict) = value.num_predict {
            s = s.num_predict(num_predict);
        }
        if let Some(top_k) = value.top_k {
            s = s.top_k(top_k);
        }
        if let Some(top_p) = value.top_p {
            s = s.top_p(top_p);
        }
        s
    }
}

impl ModelSettings {
    fn edit_numeric<N: Numeric>(
        ui: &mut egui::Ui,
        val: &mut Option<N>,
        mut default: N,
        speed: f64,
        name: &str,
        doc: &str,
    ) {
        ui.collapsing(name, |ui| {
            ui.label(doc);
            let mut enabled = val.is_some();
            ui.checkbox(&mut enabled, "Enable");

            if !enabled {
                *val = None;
            } else if val.is_none() {
                *val = Some(default);
            }

            ui.add_enabled_ui(val.is_some(), |ui| {
                ui.horizontal(|ui| {
                    if let Some(val) = val {
                        ui.add(egui::DragValue::new(val).speed(speed));
                    } else {
                        ui.add(egui::DragValue::new(&mut default).speed(speed));
                    }
                    if ui
                        .button("max")
                        .on_hover_text("Set maximum value")
                        .clicked()
                    {
                        *val = Some(N::MAX);
                    }
                    if ui
                        .button("min")
                        .on_hover_text("Set minimum value")
                        .clicked()
                    {
                        *val = Some(N::MIN);
                    }
                    if ui
                        .button("reset")
                        .on_hover_text("Set default value")
                        .clicked()
                    {
                        *val = None;
                    }
                });
            });
        });
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Mirostat", |ui| {
            ui.label("Enable Mirostat sampling for controlling perplexity. (default: 0, 0 = disabled, 1 = Mirostat, 2 = Mirostat 2.0)");
            
            let mut enabled = self.mirostat.is_some();
            ui.checkbox(&mut enabled, "Enable");
            if !enabled {
                self.mirostat = None;
            } else if self.mirostat.is_none() {
                self.mirostat = Some(Mirostat::Disabled);
            }

            ui.add_enabled_ui(self.mirostat.is_some(), |ui| {
                if let Some(mirostat) = self.mirostat {
                    egui::ComboBox::new("mirostat_combobox", "Mirostat")
                        .selected_text(mirostat.name())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.mirostat,
                                Some(Mirostat::Disabled),
                                "Disabled",
                            );
                            ui.selectable_value(
                                &mut self.mirostat,
                                Some(Mirostat::Mirostat),
                                "Mirostat",
                            );
                            ui.selectable_value(
                                &mut self.mirostat,
                                Some(Mirostat::Mirostat2),
                                "Mirostat 2.0",
                            );
                        });
                }
            });
        });

        Self::edit_numeric(ui, &mut self.mirostat_eta, 0.1, 0.01, "Mirostat eta", "Influences how quickly the algorithm responds to feedback from the generated text. A lower learning rate will result in slower adjustments, while a higher learning rate will make the algorithm more responsive.");
        Self::edit_numeric(ui, &mut self.mirostat_tau, 5.0, 0.01, "Mirostat tau", "Controls the balance between coherence and diversity of the output. A lower value will result in more focused and coherent text.");
        Self::edit_numeric(
            ui,
            &mut self.num_ctx,
            2048,
            1.0,
            "Context Window",
            "Sets the size of the context window used to generate the next token.",
        );
        Self::edit_numeric(ui, &mut self.num_gqa, 8, 1.0, "Number of GQA Groups", "The number of GQA groups in the transformer layer. Required for some models, for example it is 8 for llama2:70b.");
        Self::edit_numeric(ui, &mut self.num_gpu, 1, 1.0, "GPU Layers", "The number of layers to send to the GPU(s). On macOS it defaults to 1 to enable metal support, 0 to disable.");
        Self::edit_numeric(ui, &mut self.num_thread, 0, 1.0, "Number of Threads", "Sets the number of threads to use during computation. By default, Ollama will detect this for optimal performance. It is recommended to set this value to the number of physical CPU cores your system has (as opposed to the logical number of cores).");
        Self::edit_numeric(
            ui,
            &mut self.repeat_last_n,
            64,
            1.0,
            "Repeat Last N",
            "Sets how far back for the model to look back to prevent repetition.",
        );
        Self::edit_numeric(
            ui,
            &mut self.repeat_penalty,
            1.1,
            0.01,
            "Repeat Penalty",
            "Sets how strongly to penalize repetitions. A higher value (e.g., 1.5) will penalize repetitions more strongly, while a lower value (e.g., 0.9) will be more lenient.",
        );
        Self::edit_numeric(ui, &mut self.temperature, 0.8, 0.1, "Temperature", "The temperature of the model. Increasing the temperature will make the model answer more creatively.");
        Self::edit_numeric(ui, &mut self.seed, 0, 1.0, "Seed", "Sets the random number seed to use for generation. Setting this to a specific number will make the model generate the same text for the same prompt.");

        ui.collapsing("Stop Sequence", |ui| {
            ui.label(
                "Sets the stop sequences to use. \
                When this pattern is encountered the LLM will stop generating text and return.",
            );
            let mut enabled = self.stop.is_some();
            ui.checkbox(&mut enabled, "Enable");
            if !enabled {
                self.stop = None;
            } else if self.stop.is_none() {
                self.stop = Some(Vec::new());
            }

            ui.add_enabled_ui(self.stop.is_some(), |ui| {
                if let Some(ref mut stop) = self.stop {
                    stop.retain_mut(|pat| {
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(pat);
                            !ui.button("❌").clicked()
                        })
                        .inner
                    });
                    if stop.is_empty() {
                        ui.label("No stop sequences set, add one.");
                    }
                    ui.horizontal(|ui| {
                        if ui.button("➕ Add").clicked() {
                            stop.push(String::new());
                        }
                        if ui.button("Clear").clicked() {
                            stop.clear();
                        }
                    });
                } else {
                    let _ = ui.button("➕ Add");
                }
            });
        });

        Self::edit_numeric(ui, &mut self.tfs_z, 1.0, 0.01, "Tail-Free Sampling Z", "Tail free sampling is used to reduce the impact of less probable tokens from the output. A higher value (e.g., 2.0) will reduce the impact more, while a value of 1.0 disables this setting.");
        Self::edit_numeric(ui, &mut self.num_predict, 128, 1.0, "Number to Predict", "Maximum number of tokens to predict when generating text. (Default: 128, -1 = infinite generation, -2 = fill context)");
        Self::edit_numeric(ui, &mut self.top_k, 40, 1.0, "Top K", "Reduces the probability of generating nonsense. A higher value (e.g. 100) will give more diverse answers, while a lower value (e.g. 10) will be more conservative.");
        Self::edit_numeric(ui, &mut self.top_p, 0.9, 0.01, "Top P", "Works together with top-k. A higher value (e.g., 0.95) will lead to more diverse text, while a lower value (e.g., 0.5) will generate more focused and conservative text.");
    }
}
