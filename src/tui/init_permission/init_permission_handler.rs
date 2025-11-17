use super::init_permission_state::{
    InitialPermissionChoice, InitialPermissionDialogResult, InitialPermissionState,
};
use crate::tui::handler_result::KeyHandlerResult;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

pub struct InitialPermissionHandler {
    pub response_tx: mpsc::UnboundedSender<InitialPermissionDialogResult>,
}

impl InitialPermissionHandler {
    pub fn new(response_tx: mpsc::UnboundedSender<InitialPermissionDialogResult>) -> Self {
        Self { response_tx }
    }

    pub async fn handle_event(
        &mut self,
        event: &Event,
        state: &mut InitialPermissionState,
    ) -> KeyHandlerResult {
        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;

        if let KeyCode::Char('c') = key
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            state.should_quit = true;
            let _ = self
                .response_tx
                .send(InitialPermissionDialogResult::Cancelled);
            return KeyHandlerResult::ShouldQuit;
        }

        let choice = match key {
            KeyCode::Up => {
                state.select_prev();
                None
            }
            KeyCode::Down => {
                state.select_next();
                None
            }
            KeyCode::Enter => Some(state.get_selected_choice()),
            KeyCode::Char('1') => Some(InitialPermissionChoice::ReadOnly),
            KeyCode::Char('2') => Some(InitialPermissionChoice::EnableWriteEdit),
            KeyCode::Char('3') | KeyCode::Esc => Some(InitialPermissionChoice::Deny),
            _ => None,
        };

        if let Some(choice) = choice {
            state.should_quit = true;
            let result = match choice {
                InitialPermissionChoice::Deny => InitialPermissionDialogResult::Cancelled,
                _ => InitialPermissionDialogResult::Choice(choice),
            };
            let _ = self.response_tx.send(result);
            return KeyHandlerResult::ShouldQuit;
        }

        KeyHandlerResult::Handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventKind};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_ctrl_c_sends_cancelled() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        let result = handler.handle_event(&event, &mut state).await;
        assert!(matches!(result, KeyHandlerResult::ShouldQuit));
        assert!(state.should_quit);

        // Check that Cancelled was sent
        let received = rx.try_recv().unwrap();
        assert!(matches!(received, InitialPermissionDialogResult::Cancelled));
    }

    #[tokio::test]
    async fn test_up_arrow_selects_prev() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));
        state.selected_index = 1;

        let event = Event::Key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        let result = handler.handle_event(&event, &mut state).await;
        assert!(matches!(result, KeyHandlerResult::Handled));
        assert_eq!(state.selected_index, 0);
    }

    #[tokio::test]
    async fn test_down_arrow_selects_next() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        let result = handler.handle_event(&event, &mut state).await;
        assert!(matches!(result, KeyHandlerResult::Handled));
        assert_eq!(state.selected_index, 1);
    }

    #[tokio::test]
    async fn test_enter_sends_current_choice() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));
        state.selected_index = 0; // ReadOnly

        let event = Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        let result = handler.handle_event(&event, &mut state).await;
        assert!(matches!(result, KeyHandlerResult::ShouldQuit));
        assert!(state.should_quit);

        let received = rx.try_recv().unwrap();
        assert!(matches!(
            received,
            InitialPermissionDialogResult::Choice(InitialPermissionChoice::ReadOnly)
        ));
    }

    #[tokio::test]
    async fn test_number_1_sends_readonly() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('1'),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        handler.handle_event(&event, &mut state).await;
        let received = rx.try_recv().unwrap();
        assert!(matches!(
            received,
            InitialPermissionDialogResult::Choice(InitialPermissionChoice::ReadOnly)
        ));
    }

    #[tokio::test]
    async fn test_number_2_sends_enable_write() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('2'),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        handler.handle_event(&event, &mut state).await;
        let received = rx.try_recv().unwrap();
        assert!(matches!(
            received,
            InitialPermissionDialogResult::Choice(InitialPermissionChoice::EnableWriteEdit)
        ));
    }

    #[tokio::test]
    async fn test_number_3_sends_cancelled() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('3'),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        handler.handle_event(&event, &mut state).await;
        let received = rx.try_recv().unwrap();
        assert!(matches!(received, InitialPermissionDialogResult::Cancelled));
    }

    #[tokio::test]
    async fn test_esc_sends_cancelled() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        handler.handle_event(&event, &mut state).await;
        let received = rx.try_recv().unwrap();
        assert!(matches!(received, InitialPermissionDialogResult::Cancelled));
    }

    #[tokio::test]
    async fn test_unknown_key_not_handled() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        });

        let result = handler.handle_event(&event, &mut state).await;
        assert!(matches!(result, KeyHandlerResult::Handled));
        assert!(!state.should_quit);
    }

    #[tokio::test]
    async fn test_non_key_event_not_handled() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut handler = InitialPermissionHandler::new(tx);
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));

        let event = Event::Resize(80, 24);
        let result = handler.handle_event(&event, &mut state).await;
        assert!(matches!(result, KeyHandlerResult::NotHandled));
    }
}
