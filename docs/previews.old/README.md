# TUI Visual Previews

This folder keeps the design references and generated Viden TUI preview artifacts.

Reference images:

- `viden-tui-system-screenshot.svg`
- `tui-deepseek-reference-v1.png`
- `tui-concept-holodeck-v1.png`
- `tui-multiscreen-agents-v1.png`
- `tui-theme-variants-v1.png`
- `tui-desk-*.png`

Generate current terminal previews:

```bash
scripts/tui-previews.sh
```

The script writes:

- `generated/main.txt` and `generated/main.ansi`
- `generated/main-lane.txt` and `generated/main-lane.ansi`
- `generated/main.svg` and `generated/main-lane.svg`
- `generated/side-1.txt` and `generated/side-1.ansi`
- `generated/side-2.txt` and `generated/side-2.ansi`
- `generated/multiscreen.txt`
- `generated/main.<theme>.ansi` for each built-in theme variant

Optional environment overrides:

```bash
VIDEN_TUI_PREVIEW_THEME=ember-gold scripts/tui-previews.sh
VIDEN_TUI_PREVIEW_THEMES="aurora-cyan ember-gold" scripts/tui-previews.sh
VIDEN_TUI_PREVIEW_PROVIDER=fallback VIDEN_TUI_PREVIEW_MODEL=test-local scripts/tui-previews.sh
```

Generated previews intentionally use the live workspace snapshot, so recent-file rows can change after edits or after regenerating previews.
