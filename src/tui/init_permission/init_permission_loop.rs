use super::init_permission_handler::InitialPermissionHandler;
use super::init_permission_layout::InitialPermissionLayout;
use super::init_permission_state::{
    InitialPermissionChoice, InitialPermissionDialogResult, InitialPermissionState,
};
use crate::permissions::storage::{PermissionRule, PermissionsFile};
use crate::tools::ToolRegistry;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::layout::Layout;
use crate::tui::terminal::{HooshTerminal, resize_terminal};
use anyhow::Result;
use crossterm::event;
use std::path::PathBuf;
use tokio::time::Duration;

pub async fn run(
    terminal: HooshTerminal,
    project_root: PathBuf,
    tool_registry: &ToolRegistry,
    skip_permissions: bool,
) -> Result<(HooshTerminal, InitialPermissionDialogResult)> {
    // Check if we should show the initial permission dialog
    let permissions_path = PermissionsFile::get_permissions_path(&project_root);
    let should_show_initial_dialog = !skip_permissions && !permissions_path.exists();

    if !should_show_initial_dialog {
        return Ok((
            terminal,
            InitialPermissionDialogResult::SkippedPermissionsExist,
        ));
    }

    let mut app = InitialPermissionState::new(project_root.clone());

    let (terminal, result) = run_dialog_loop(terminal, &mut app).await;

    if let InitialPermissionDialogResult::Choice(ref choice) = result {
        save_permission_choice(choice, &project_root, tool_registry)?;
    }

    Ok((terminal, result))
}

async fn run_dialog_loop(
    mut terminal: HooshTerminal,
    app: &mut InitialPermissionState,
) -> (HooshTerminal, InitialPermissionDialogResult) {
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
            let handler_result = handler.handle_event(&event, app).await;
            if matches!(handler_result, KeyHandlerResult::ShouldQuit) {
                if let Ok(result) = response_rx.try_recv() {
                    return (terminal, result);
                }
                return (terminal, InitialPermissionDialogResult::Cancelled);
            }
        }

        if let Ok(result) = response_rx.try_recv() {
            return (terminal, result);
        }

        if app.should_quit {
            return (terminal, InitialPermissionDialogResult::Cancelled);
        }
    }
}

fn save_permission_choice(
    choice: &InitialPermissionChoice,
    project_root: &std::path::Path,
    tool_registry: &ToolRegistry,
) -> Result<()> {
    let mut perms_file = PermissionsFile::default();

    match choice {
        InitialPermissionChoice::ReadOnly => {
            perms_file.save_permissions(project_root)?;
        }
        InitialPermissionChoice::EnableWriteEdit => {
            for (tool_name, _) in tool_registry.list_tools() {
                if let Some(tool) = tool_registry.get_tool(tool_name) {
                    let descriptor = tool.describe_permission(None);
                    if descriptor.is_destructive() || descriptor.is_write_safe() {
                        perms_file.add_permission(PermissionRule::ops_rule(tool_name, "*"), true);
                    }
                }
            }
            perms_file.save_permissions(project_root)?;
        }
        InitialPermissionChoice::Deny => {}
    }

    Ok(())
}
