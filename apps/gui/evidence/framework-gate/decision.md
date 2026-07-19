# GUI Framework Gate Decision Evidence

Selected framework: **tauri**.

Rule: any non-pass GPUI measurement or hard gate selects Tauri. An unavailable or partial result is not treated as a pass.

## GPUI blockers

- `measurements.composer_input_p95_ms:unavailable`
- `measurements.event_to_visible_p95_ms:unavailable`
- `measurements.frame_work_p95_ms:unavailable`
- `hard_gates.cjk_ime:partial`
- `hard_gates.keyboard_only:partial`
- `hard_gates.screen_reader:unavailable`
- `hard_gates.linux_build_launch:unavailable`
- `hard_gates.windows_build_launch:unavailable`
- `hard_gates.bounded_transcript_rendering:partial`
- `hard_gates.bounded_soak:unavailable`
- `hard_gates.idle_cpu_near_zero:unavailable`
- `hard_gates.visual_parity:unavailable`
- `hard_gates.signing:unavailable`
- `hard_gates.updater:unavailable`
- `hard_gates.credential_storage:unavailable`
- `hard_gates.crash_recovery:unavailable`

## Tauri limitations retained for the production gate

- `measurements.composer_input_p95_ms:unavailable`
- `measurements.event_to_visible_p95_ms:unavailable`
- `measurements.frame_work_p95_ms:unavailable`
- `hard_gates.cjk_ime:partial`
- `hard_gates.keyboard_only:partial`
- `hard_gates.screen_reader:unavailable`
- `hard_gates.linux_build_launch:unavailable`
- `hard_gates.windows_build_launch:unavailable`
- `hard_gates.bounded_transcript_rendering:partial`
- `hard_gates.bounded_soak:unavailable`
- `hard_gates.idle_cpu_near_zero:unavailable`
- `hard_gates.visual_parity:unavailable`
- `hard_gates.signing:unavailable`
- `hard_gates.updater:unavailable`
- `hard_gates.credential_storage:unavailable`
- `hard_gates.crash_recovery:unavailable`

Candidate records: [Tauri](tauri.json) and [GPUI](gpui.json).
