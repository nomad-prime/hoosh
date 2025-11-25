use crate::agent_definition::AgentDefinitionManager;
use crate::config::AppConfig;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::layout::Layout;
use crate::tui::setup::setup_wizard_handler::SetupWizardHandler;
use crate::tui::setup::setup_wizard_layout::SetupWizardLayout;
use crate::tui::setup::setup_wizard_state::{SetupWizardResult, SetupWizardState};
use crate::tui::terminal::{HooshTerminal, resize_terminal};
use anyhow::Result;
use crossterm::event;
use tokio::time::Duration;

pub async fn run(terminal: HooshTerminal) -> Result<(HooshTerminal, Option<SetupWizardResult>)> {
    let mut app = SetupWizardState::new();

    let (terminal, result) = run_wizard_loop(terminal, &mut app).await;

    Ok((terminal, result))
}

async fn run_wizard_loop(
    mut terminal: HooshTerminal,
    app: &mut SetupWizardState,
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
            if matches!(handler_result, KeyHandlerResult::ShouldQuit) {
                if let Ok(result) = response_rx.try_recv() {
                    return (terminal, result);
                }
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
    let config_path = AppConfig::config_path()?;

    let mut config = AppConfig::load().unwrap_or_else(|_| AppConfig::default());

    config.default_backend = result.backend.clone();

    config.backends.insert(
        result.backend.clone(),
        crate::config::BackendConfig {
            api_key: result.api_key.clone(),
            model: Some(result.model.clone()),
            base_url: result.base_url.clone(),
            chat_api: None,
            temperature: None,
        },
    );

    config.save()?;

    if !config_path.exists() {
        return Err(anyhow::anyhow!(
            "Config file not found after save operation at {}",
            config_path.display()
        ));
    }

    let agents_dir = AppConfig::agents_dir()?;
    AgentDefinitionManager::initialize_default_agents(&agents_dir)?;

    Ok(())
}
