use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum InitialPermissionChoice {
    ReadOnly,
    EnableWriteEdit,
    Deny,
}

#[derive(Clone, Debug)]
pub enum InitialPermissionDialogResult {
    SkippedPermissionsExist,
    Choice(InitialPermissionChoice),
    Cancelled,
}

pub struct InitialPermissionState {
    pub project_root: PathBuf,
    pub selected_index: usize,
    pub should_quit: bool,
}

impl InitialPermissionState {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            selected_index: 0,
            should_quit: false,
        }
    }

    pub fn select_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % 3;
    }

    pub fn select_prev(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = 2;
        } else {
            self.selected_index -= 1;
        }
    }

    pub fn get_selected_choice(&self) -> InitialPermissionChoice {
        match self.selected_index {
            0 => InitialPermissionChoice::ReadOnly,
            1 => InitialPermissionChoice::EnableWriteEdit,
            2 => InitialPermissionChoice::Deny,
            _ => InitialPermissionChoice::ReadOnly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = InitialPermissionState::new(PathBuf::from("/test"));
        assert_eq!(state.selected_index, 0);
        assert!(!state.should_quit);
    }

    #[test]
    fn test_select_next_cycles() {
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));
        assert_eq!(state.selected_index, 0);

        state.select_next();
        assert_eq!(state.selected_index, 1);

        state.select_next();
        assert_eq!(state.selected_index, 2);

        state.select_next();
        assert_eq!(state.selected_index, 0); // Cycles back
    }

    #[test]
    fn test_select_prev_cycles() {
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));
        state.select_prev();
        assert_eq!(state.selected_index, 2); // Cycles to end

        state.select_prev();
        assert_eq!(state.selected_index, 1);

        state.select_prev();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_get_selected_choice_readonly() {
        let state = InitialPermissionState::new(PathBuf::from("/test"));
        match state.get_selected_choice() {
            InitialPermissionChoice::ReadOnly => (),
            _ => panic!("Expected ReadOnly"),
        }
    }

    #[test]
    fn test_get_selected_choice_enable_write() {
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));
        state.selected_index = 1;
        match state.get_selected_choice() {
            InitialPermissionChoice::EnableWriteEdit => (),
            _ => panic!("Expected EnableWriteEdit"),
        }
    }

    #[test]
    fn test_get_selected_choice_deny() {
        let mut state = InitialPermissionState::new(PathBuf::from("/test"));
        state.selected_index = 2;
        match state.get_selected_choice() {
            InitialPermissionChoice::Deny => (),
            _ => panic!("Expected Deny"),
        }
    }
}
