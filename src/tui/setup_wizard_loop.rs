use crate::config::AppConfig;
use crate::tui::handlers::SetupWizardHandler;
use crate::tui::layout::Layout;
use crate::tui::setup_wizard_app::{SetupWizardApp, SetupWizardResult};
use crate::tui::setup_wizard_layout::SetupWizardLayout;
use crate::tui::terminal::{HooshTerminal, resize_terminal};
use anyhow::Result;
use crossterm::event;
use std::collections::HashMap;
use tokio::time::Duration;

pub async fn run(terminal: HooshTerminal) -> Result<(HooshTerminal, Option<SetupWizardResult>)> {
    let mut app = SetupWizardApp::new();

    let (terminal, result) = run_wizard_loop(terminal, &mut app).await;

    Ok((terminal, result))
}

async fn run_wizard_loop(
    mut terminal: HooshTerminal,
    app: &mut SetupWizardApp,
) -> (HooshTerminal, Option<SetupWizardResult>) {
    let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut handler = SetupWizardHandler::new(response_tx);

    loop {
        let layout = Layout::create(app);

        resize_terminal(&mut terminal, layout.total_height()).expect("could not resize terminal");

        terminal
            .draw(|frame| {
                layout.render(app, frame.area(), frame.buffer_mut());
            })
            .expect("could not draw terminal");

        if event::poll(Duration::from_millis(100)).expect("could not poll events") {
            let event = event::read().expect("could not read event");
            let handler_result = handler.handle_event(&event, app).await;
            use crate::tui::handler_result::KeyHandlerResult;
            if matches!(handler_result, KeyHandlerResult::ShouldQuit) {
                return (terminal, None);
            }
        }

        if let Ok(result) = response_rx.try_recv() {
            return (terminal, result);
        }

        if app.should_quit {
            return (terminal, None);
        }
    }
}

pub fn save_wizard_result(result: &SetupWizardResult) -> Result<()> {
    let mut config = AppConfig::load().unwrap_or_default();

    config.default_backend = result.backend.clone();

    let mut backend_config = HashMap::new();
    if let Some(api_key) = &result.api_key {
        backend_config.insert("api_key".to_string(), api_key.clone());
    } else {
        let env_var_name = format!(
            "{}_API_KEY",
            result.backend.to_uppercase().replace("-", "_")
        );
        backend_config.insert("api_key".to_string(), format!("${{{}}}", env_var_name));
    }
    backend_config.insert("model".to_string(), result.model.clone());

    config.backends.insert(
        result.backend.clone(),
        crate::config::BackendConfig {
            api_key: if result.api_key.is_some() {
                result.api_key.clone()
            } else {
                None
            },
            model: Some(result.model.clone()),
            base_url: None,
            chat_api: None,
            temperature: None,
        },
    );

    config.save()?;

    Ok(())
}
