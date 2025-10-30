use crate::permissions::storage::{PermissionRule, PermissionsFile};
use crate::tools::{BuiltinToolProvider, ToolRegistry};
use crate::tui::app::{AppState, InitialPermissionChoice};
use crate::tui::handlers::InitialPermissionHandler;
use crate::tui::initial_permission_layout::InitialPermissionLayout;
use crate::tui::input_handler::InputHandler;
use crate::tui::layout::Layout;
use crate::tui::terminal::{resize_terminal, HooshTerminal};
use anyhow::Result;
use crossterm::event;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;

pub async fn run(
    terminal: HooshTerminal,
    project_root: PathBuf,
) -> Result<(HooshTerminal, Option<InitialPermissionChoice>)> {
    let mut app = AppState::new();
    app.show_initial_permission_dialog(project_root.clone());

    let (terminal, choice) = run_dialog_loop(terminal, &mut app).await;

    if let Some(ref choice) = choice {
        save_permission_choice(choice, &project_root)?;
    }

    Ok((terminal, choice))
}

async fn run_dialog_loop(
    mut terminal: HooshTerminal,
    app: &mut AppState,
) -> (HooshTerminal, Option<InitialPermissionChoice>) {
    let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut handler = InitialPermissionHandler::new(response_tx);

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
            if let Ok(handler_result) = handler.handle_event(&event, app, false).await {
                use crate::tui::handler_result::KeyHandlerResult;
                if matches!(handler_result, KeyHandlerResult::ShouldQuit) {
                    return (terminal, None);
                }
            }
        }

        if let Ok(choice) = response_rx.try_recv() {
            return (terminal, Some(choice));
        }

        if app.should_quit {
            return (terminal, None);
        }
    }
}

fn save_permission_choice(choice: &InitialPermissionChoice, project_root: &PathBuf) -> Result<()> {
    let mut perms_file = PermissionsFile::default();

    let registry =
        ToolRegistry::new().with_provider(Arc::new(BuiltinToolProvider::new(project_root.clone())));

    match choice {
        InitialPermissionChoice::ReadOnly => {
            for (tool_name, _) in registry.list_tools() {
                if let Some(tool) = registry.get_tool(tool_name) {
                    if tool.read_only() {
                        perms_file.add_permission(PermissionRule::ops_rule(tool_name, "*"), true);
                    }
                }
            }
            perms_file.save_permissions(project_root)?;
        }
        InitialPermissionChoice::TrustProject => {
            for (tool_name, _) in registry.list_tools() {
                perms_file.add_permission(PermissionRule::ops_rule(tool_name, "*"), true);
            }
            perms_file.save_permissions(project_root)?;
        }
        InitialPermissionChoice::Deny => {}
    }

    Ok(())
}
