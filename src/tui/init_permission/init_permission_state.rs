use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum InitialPermissionChoice {
    ReadOnly,
    EnableWriteEdit,
    Deny,
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
