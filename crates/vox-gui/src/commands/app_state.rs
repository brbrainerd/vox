use std::sync::Mutex;
use tauri::State;

pub struct GuiState {
    pub initial_view: Mutex<Option<String>>,
}

#[tauri::command]
pub fn get_initial_view(state: State<'_, GuiState>) -> Option<String> {
    state.initial_view.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_state_holds_initial_view() {
        let state = GuiState {
            initial_view: Mutex::new(Some("dashboard".to_string())),
        };
        assert_eq!(
            state.initial_view.lock().unwrap().as_deref(),
            Some("dashboard")
        );
    }
}
