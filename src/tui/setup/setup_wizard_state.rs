use crate::tui::palette;
use ratatui::style::{Modifier, Style};
use tui_textarea::TextArea;

#[derive(Debug, Clone)]
pub enum SetupWizardStep {
    Welcome,
    BackendSelection,
    ApiKeyInput,
    ModelSelection,
    Confirmation,
}

#[derive(Debug, Clone)]
pub enum BackendType {
    Anthropic,
    OpenAI,
    TogetherAI,
    Ollama,
}

impl BackendType {
    pub fn as_str(&self) -> &str {
        match self {
            BackendType::Anthropic => "anthropic",
            BackendType::OpenAI => "openai",
            BackendType::TogetherAI => "together_ai",
            BackendType::Ollama => "ollama",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            BackendType::Anthropic => "Claude AI models (Sonnet, Opus, Haiku)",
            BackendType::OpenAI => "GPT models (GPT-4, GPT-3.5)",
            BackendType::TogetherAI => "Various open source models",
            BackendType::Ollama => "Local LLM inference (no API key needed)",
        }
    }

    pub fn default_model(&self) -> &str {
        match self {
            BackendType::Anthropic => "claude-sonnet-4-20250514",
            BackendType::OpenAI => "gpt-4",
            BackendType::TogetherAI => "meta-llama/Llama-3-70b-chat-hf",
            BackendType::Ollama => "llama3",
        }
    }

    pub fn needs_api_key(&self) -> bool {
        !matches!(self, BackendType::Ollama)
    }

    pub fn all_backends() -> Vec<BackendType> {
        vec![
            BackendType::Anthropic,
            BackendType::OpenAI,
            BackendType::TogetherAI,
            BackendType::Ollama,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct SetupWizardResult {
    pub backend: String,
    pub api_key: Option<String>,
    pub model: String,
    pub store_key_in_config: bool,
}

pub struct SetupWizardState {
    pub current_step: SetupWizardStep,
    pub selected_backend_index: usize,
    pub selected_backend: Option<BackendType>,
    pub api_key_input: TextArea<'static>,
    pub model_input: TextArea<'static>,
    pub selected_confirmation_index: usize,
    pub should_quit: bool,
    pub result: Option<SetupWizardResult>,
}

impl Default for SetupWizardState {
    fn default() -> Self {
        let mut api_key_input = TextArea::default();
        api_key_input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        api_key_input.set_cursor_line_style(Style::default());
        api_key_input.set_placeholder_text("Enter API key");
        api_key_input.set_placeholder_style(Style::default().fg(palette::PLACEHOLDER));

        let mut model_input = TextArea::default();
        model_input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        model_input.set_cursor_line_style(Style::default());
        model_input.set_placeholder_text("Enter Model name");
        model_input.set_placeholder_style(Style::default().fg(palette::PLACEHOLDER));

        Self {
            current_step: SetupWizardStep::Welcome,
            selected_backend_index: 0,
            selected_backend: None,
            api_key_input,
            model_input,
            selected_confirmation_index: 0,
            should_quit: false,
            result: None,
        }
    }
}

impl SetupWizardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select_next_backend(&mut self) {
        let backends = BackendType::all_backends();
        self.selected_backend_index = (self.selected_backend_index + 1) % backends.len();
    }

    pub fn select_prev_backend(&mut self) {
        let backends = BackendType::all_backends();
        if self.selected_backend_index == 0 {
            self.selected_backend_index = backends.len() - 1;
        } else {
            self.selected_backend_index -= 1;
        }
    }

    pub fn confirm_backend_selection(&mut self) {
        let backends = BackendType::all_backends();
        self.selected_backend = Some(backends[self.selected_backend_index].clone());
    }

    pub fn advance_step(&mut self) {
        self.current_step = match &self.current_step {
            SetupWizardStep::Welcome => SetupWizardStep::BackendSelection,
            SetupWizardStep::BackendSelection => {
                self.confirm_backend_selection();
                if let Some(backend) = &self.selected_backend {
                    if backend.needs_api_key() {
                        SetupWizardStep::ApiKeyInput
                    } else {
                        self.model_input.insert_str(backend.default_model());
                        SetupWizardStep::ModelSelection
                    }
                } else {
                    SetupWizardStep::BackendSelection
                }
            }
            SetupWizardStep::ApiKeyInput => {
                if let Some(backend) = &self.selected_backend {
                    self.model_input.select_all();
                    self.model_input.cut();
                    self.model_input.insert_str(backend.default_model());
                }
                SetupWizardStep::ModelSelection
            }
            SetupWizardStep::ModelSelection => SetupWizardStep::Confirmation,
            SetupWizardStep::Confirmation => SetupWizardStep::Confirmation,
        };
    }

    pub fn go_back(&mut self) {
        self.current_step = match &self.current_step {
            SetupWizardStep::Welcome => SetupWizardStep::Welcome,
            SetupWizardStep::BackendSelection => SetupWizardStep::Welcome,
            SetupWizardStep::ApiKeyInput => SetupWizardStep::BackendSelection,
            SetupWizardStep::ModelSelection => {
                if let Some(backend) = &self.selected_backend {
                    if backend.needs_api_key() {
                        SetupWizardStep::ApiKeyInput
                    } else {
                        SetupWizardStep::BackendSelection
                    }
                } else {
                    SetupWizardStep::BackendSelection
                }
            }
            SetupWizardStep::Confirmation => SetupWizardStep::ModelSelection,
        };
    }

    pub fn select_next_confirmation_option(&mut self) {
        self.selected_confirmation_index = (self.selected_confirmation_index + 1) % 2;
    }

    pub fn select_prev_confirmation_option(&mut self) {
        if self.selected_confirmation_index == 0 {
            self.selected_confirmation_index = 1;
        } else {
            self.selected_confirmation_index = 0;
        }
    }

    pub fn confirm_setup(&mut self) {
        if let Some(backend) = &self.selected_backend {
            let api_key = if backend.needs_api_key() {
                let key_text = self.api_key_input.lines()[0].clone();
                if key_text.is_empty() {
                    None
                } else {
                    Some(key_text)
                }
            } else {
                None
            };

            let model = self.model_input.lines()[0].clone();

            self.result = Some(SetupWizardResult {
                backend: backend.as_str().to_string(),
                api_key,
                model,
                store_key_in_config: true,
            });
        }
    }

    pub fn cancel_setup(&mut self) {
        self.should_quit = true;
    }
}
