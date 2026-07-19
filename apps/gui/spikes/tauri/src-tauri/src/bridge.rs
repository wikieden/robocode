use viden_gui_spike_common::D1FixtureProjection;

#[tauri::command]
pub fn d1_fixture_projection() -> Result<D1FixtureProjection, String> {
    D1FixtureProjection::from_committed_fixture()
}

/// Keeps framework registration behind a Viden-owned boundary so the spike's
/// D1 model does not depend on Tauri widgets or runtime internals.
pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default().invoke_handler(tauri::generate_handler![d1_fixture_projection])
}

#[cfg(test)]
mod tests {
    use super::d1_fixture_projection;

    #[test]
    fn bridge_returns_the_core_reduced_fixture_projection() {
        let projection = d1_fixture_projection().unwrap();

        assert_eq!(projection.project_id, "project_viden");
        assert_eq!(projection.lane_id, "lane_d1_core");
        assert_eq!(
            projection.view_hash,
            "7dd8faf04cca9f3013198e25823894eae91c2869e27087aa1eb0a34890cdf804"
        );
    }
}
