use anyhow::Result;

use super::app::AppState;
use super::terminal::{resize_terminal, Tui};

/// Height constants for different UI elements
const BASE_HEIGHT: u16 = 6;
const PERMISSION_DIALOG_HEIGHT: u16 = 15;
const APPROVAL_DIALOG_HEIGHT: u16 = 10;
const COMPLETION_POPUP_HEIGHT: u16 = 12;

/// Manages viewport height based on dialog/popup visibility.
///
/// The viewport height is UI-driven and adjusts dynamically to accommodate:
/// - Base height (6 lines): Default chat view
/// - Permission dialog (+15 lines): Space for permission options
/// - Approval dialog (+10 lines): Space for approval choices
/// - Completion popup (+12 lines): Space for file/command completions
pub struct ViewportManager {
    base_height: u16,
    current_height: u16,
}

impl ViewportManager {
    /// Creates a new ViewportManager with the specified base height.
    pub fn new(base_height: u16) -> Self {
        Self {
            base_height,
            current_height: base_height,
        }
    }

    /// Creates a new ViewportManager with the default base height (6 lines).
    pub fn with_default_height() -> Self {
        Self::new(BASE_HEIGHT)
    }

    /// Calculates the required viewport height based on the current app state.
    ///
    /// Returns the base height plus any additional height needed for visible dialogs/popups.
    pub fn calculate_required_height(&self, app: &AppState) -> u16 {
        let dialog_height = if app.is_showing_permission_dialog() {
            PERMISSION_DIALOG_HEIGHT
        } else if app.is_showing_approval_dialog() {
            APPROVAL_DIALOG_HEIGHT
        } else if app.is_completing() {
            COMPLETION_POPUP_HEIGHT
        } else {
            0
        };

        self.base_height + dialog_height
    }

    /// Returns the current viewport height.
    #[allow(dead_code)]
    pub fn current_height(&self) -> u16 {
        self.current_height
    }

    /// Checks if the viewport needs to be resized based on the current app state.
    #[allow(dead_code)]
    pub fn needs_resize(&self, app: &AppState) -> bool {
        self.calculate_required_height(app) != self.current_height
    }

    /// Updates the viewport height based on app state and resizes the terminal if needed.
    ///
    /// Returns the terminal (possibly resized) and a bool indicating if a resize occurred.
    pub fn update_and_resize(&mut self, app: &AppState, terminal: Tui) -> Result<(Tui, bool)> {
        let required_height = self.calculate_required_height(app);

        if required_height != self.current_height {
            let resized_terminal = resize_terminal(terminal, required_height)?;
            self.current_height = required_height;
            Ok((resized_terminal, true))
        } else {
            Ok((terminal, false))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_height() {
        let manager = ViewportManager::with_default_height();
        assert_eq!(manager.current_height(), BASE_HEIGHT);
    }

    #[test]
    fn test_calculate_base_height_no_dialogs() {
        let manager = ViewportManager::with_default_height();
        let app = AppState::new();

        let height = manager.calculate_required_height(&app);
        assert_eq!(height, BASE_HEIGHT);
    }

    #[test]
    fn test_calculate_height_with_permission_dialog() {
        let manager = ViewportManager::with_default_height();
        let mut app = AppState::new();

        // Show permission dialog
        app.show_permission_dialog(
            crate::permissions::OperationType::ReadFile("/test/file".into()),
            "req123".to_string(),
        );

        let height = manager.calculate_required_height(&app);
        assert_eq!(height, BASE_HEIGHT + PERMISSION_DIALOG_HEIGHT);
    }

    #[test]
    fn test_calculate_height_with_approval_dialog() {
        let manager = ViewportManager::with_default_height();
        let mut app = AppState::new();

        // Show approval dialog
        app.show_approval_dialog("call123".to_string(), "test_tool".to_string());

        let height = manager.calculate_required_height(&app);
        assert_eq!(height, BASE_HEIGHT + APPROVAL_DIALOG_HEIGHT);
    }

    #[test]
    fn test_calculate_height_with_completion() {
        let manager = ViewportManager::with_default_height();
        let mut app = AppState::new();

        // Start completion
        app.start_completion(0, 0);

        let height = manager.calculate_required_height(&app);
        assert_eq!(height, BASE_HEIGHT + COMPLETION_POPUP_HEIGHT);
    }

    #[test]
    fn test_needs_resize_true_when_height_changes() {
        let manager = ViewportManager::with_default_height();
        let mut app = AppState::new();

        assert!(!manager.needs_resize(&app)); // No resize needed initially

        // Show permission dialog
        app.show_permission_dialog(
            crate::permissions::OperationType::ReadFile("/test/file".into()),
            "req123".to_string(),
        );

        assert!(manager.needs_resize(&app)); // Resize needed now
    }

    #[test]
    fn test_needs_resize_false_when_height_unchanged() {
        let manager = ViewportManager::with_default_height();
        let app = AppState::new();

        assert!(!manager.needs_resize(&app));
    }

    #[test]
    fn test_permission_dialog_takes_precedence() {
        let manager = ViewportManager::with_default_height();
        let mut app = AppState::new();

        // Show permission dialog
        app.show_permission_dialog(
            crate::permissions::OperationType::ReadFile("/test/file".into()),
            "req123".to_string(),
        );
        // Try to show approval dialog (should be ignored in height calculation)
        app.show_approval_dialog("call123".to_string(), "test_tool".to_string());

        let height = manager.calculate_required_height(&app);
        // Permission dialog should take precedence
        assert_eq!(height, BASE_HEIGHT + PERMISSION_DIALOG_HEIGHT);
    }

    #[test]
    fn test_custom_base_height() {
        let manager = ViewportManager::new(10);
        assert_eq!(manager.current_height(), 10);

        let app = AppState::new();
        let height = manager.calculate_required_height(&app);
        assert_eq!(height, 10);
    }
}
