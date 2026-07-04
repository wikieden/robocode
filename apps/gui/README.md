# viden-gui

`viden-gui` is the future desktop GUI app boundary. It is currently a minimal
contract-replay scaffold so GUI and TUI development can share the same runtime
facts before the Tauri/Web shell lands.

## Purpose

- Consume `RuntimeSnapshot`, `RuntimeEvent`, `RuntimeCommand`, and
  `RuntimeViewState` through `viden-core`.
- Provide GUI fixture replay tests before any visual implementation.
- Keep provider loops, tool execution, permission checks, transcript state,
  workflow state, and lane orchestration outside the GUI.

## Does Not Own

- Provider/model execution.
- Tool or shell execution.
- Permission decisions.
- Transcript/session/workflow persistence.
- Runtime business state.

## Test

```bash
cargo test -p viden-gui
```
