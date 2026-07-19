pub mod bridge;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    bridge::builder()
        .run(tauri::generate_context!())
        .expect("failed to run the Viden Tauri D1 spike");
}
