/// Keeps framework registration behind a Viden-owned boundary so the spike's
/// D1 model does not depend on Tauri widgets or runtime internals.
pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
}
