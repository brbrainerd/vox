use std::sync::Mutex;
use tauri::State;

pub struct GuiState {
    pub initial_view: Mutex<Option<String>>,
}

#[tauri::command]
pub fn get_initial_view(state: State<'_, GuiState>) -> Option<String> {
    state.initial_view.lock().unwrap().clone()
}
