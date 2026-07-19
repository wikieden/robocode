#!/usr/bin/env python3
"""Validate reproducible framework evidence and apply the GUI hard-gate rule."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


REQUIRED_MEASUREMENTS = {
    "composer_input_p95_ms": {"operator": "lt", "value": 50, "unit": "ms"},
    "event_to_visible_p95_ms": {"operator": "lt", "value": 100, "unit": "ms"},
    "frame_work_p95_ms": {"operator": "lt", "value": 16, "unit": "ms"},
    "ordered_event_count": {"operator": "gte", "value": 10_000, "unit": "events"},
    "transcript_row_count": {"operator": "gte", "value": 50_000, "unit": "rows"},
}

REQUIRED_HARD_GATES = (
    "d1_slice_parity",
    "cjk_ime",
    "keyboard_only",
    "screen_reader",
    "macos_build_launch",
    "linux_build_launch",
    "windows_build_launch",
    "bounded_transcript_rendering",
    "bounded_soak",
    "idle_cpu_near_zero",
    "no_long_lived_framework_fork",
    "visual_parity",
    "signing",
    "updater",
    "credential_storage",
    "crash_recovery",
)

ALLOWED_STATUSES = {"pass", "fail", "partial", "unavailable"}


class EvidenceError(ValueError):
    """Raised when candidate evidence is incomplete or self-contradictory."""


def _require_mapping(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{field} must be an object")
    return value


def _validate_status(record: dict[str, Any], field: str) -> str:
    status = record.get("status")
    if status not in ALLOWED_STATUSES:
        raise EvidenceError(f"{field}.status must be one of {sorted(ALLOWED_STATUSES)}")
    evidence = record.get("evidence")
    if not isinstance(evidence, list) or not all(isinstance(item, str) for item in evidence):
        raise EvidenceError(f"{field}.evidence must be a list of strings")
    if status == "pass" and not evidence:
        raise EvidenceError(f"{field}: pass requires evidence")
    return status


def _threshold_passes(value: float, threshold: dict[str, Any]) -> bool:
    operator = threshold["operator"]
    target = threshold["value"]
    if operator == "lt":
        return value < target
    if operator == "gte":
        return value >= target
    raise EvidenceError(f"unsupported threshold operator: {operator}")


def validate_evidence(candidate: dict[str, Any], source: str) -> None:
    if candidate.get("schema_version") != 1:
        raise EvidenceError(f"{source}: schema_version must be 1")
    if candidate.get("framework") not in {"tauri", "gpui"}:
        raise EvidenceError(f"{source}: framework must be tauri or gpui")
    if candidate.get("component_version") != "0.1.0-alpha.1":
        raise EvidenceError(f"{source}: component_version must be 0.1.0-alpha.1")

    fixture = _require_mapping(candidate.get("fixture"), f"{source}.fixture")
    for field in ("path", "sha256", "projection_sha256"):
        if not isinstance(fixture.get(field), str) or not fixture[field]:
            raise EvidenceError(f"{source}.fixture.{field} must be a non-empty string")
    _require_mapping(candidate.get("host"), f"{source}.host")
    commands = candidate.get("commands")
    if not isinstance(commands, list):
        raise EvidenceError(f"{source}.commands must be a list")
    command_names: set[str] = set()
    commands_by_name: dict[str, dict[str, Any]] = {}
    for index, raw_command in enumerate(commands):
        field = f"{source}.commands[{index}]"
        command = _require_mapping(raw_command, field)
        name = command.get("name")
        if not isinstance(name, str) or not name:
            raise EvidenceError(f"{field}.name must be a non-empty string")
        if name in command_names:
            raise EvidenceError(f"{field}.name is duplicated: {name}")
        command_names.add(name)
        commands_by_name[name] = command
        if not isinstance(command.get("command"), str) or not command["command"]:
            raise EvidenceError(f"{field}.command must be a non-empty string")
        if command.get("status") not in {"pass", "fail"}:
            raise EvidenceError(f"{field}.status must be pass or fail")
        exit_code = command.get("exit_code")
        if not isinstance(exit_code, int) or isinstance(exit_code, bool):
            raise EvidenceError(f"{field}.exit_code must be an integer")
        if (exit_code == 0) != (command["status"] == "pass"):
            raise EvidenceError(f"{field}.status contradicts exit_code")

    measurements = _require_mapping(candidate.get("measurements"), f"{source}.measurements")
    for name, expected_threshold in REQUIRED_MEASUREMENTS.items():
        field = f"{source}.measurements.{name}"
        record = _require_mapping(measurements.get(name), field)
        status = _validate_status(record, field)
        if record.get("threshold") != expected_threshold:
            raise EvidenceError(f"{field}.threshold does not match the framework gate")
        if record.get("unit") != expected_threshold["unit"]:
            raise EvidenceError(f"{field}.unit does not match the framework gate")
        value = record.get("value")
        if status == "pass":
            if not isinstance(value, (int, float)) or isinstance(value, bool):
                raise EvidenceError(f"{field}: pass requires a numeric value")
            if not _threshold_passes(value, expected_threshold):
                raise EvidenceError(f"{field}: pass contradicts its threshold")

    hard_gates = _require_mapping(candidate.get("hard_gates"), f"{source}.hard_gates")
    for name in REQUIRED_HARD_GATES:
        field = f"{source}.hard_gates.{name}"
        record = _require_mapping(hard_gates.get(name), field)
        _validate_status(record, field)
        if not isinstance(record.get("summary"), str) or not record["summary"]:
            raise EvidenceError(f"{field}.summary must be a non-empty string")

    records = [*measurements.values(), *hard_gates.values()]
    for record in records:
        for evidence in record["evidence"]:
            if not evidence.startswith("command:"):
                continue
            command_name = evidence.removeprefix("command:")
            if command_name not in command_names:
                raise EvidenceError(f"{source}: unknown command evidence: {evidence}")
            command = commands_by_name[command_name]
            if record["status"] == "pass" and (
                command["status"] != "pass" or command["exit_code"] != 0
            ):
                raise EvidenceError(f"{source}: pass record cites failed command evidence: {evidence}")


def _blockers(candidate: dict[str, Any]) -> list[str]:
    blockers = [
        f"measurements.{name}:{candidate['measurements'][name]['status']}"
        for name in REQUIRED_MEASUREMENTS
        if candidate["measurements"][name]["status"] != "pass"
    ]
    blockers.extend(
        f"hard_gates.{name}:{candidate['hard_gates'][name]['status']}"
        for name in REQUIRED_HARD_GATES
        if candidate["hard_gates"][name]["status"] != "pass"
    )
    return blockers


def compare_evidence(tauri: dict[str, Any], gpui: dict[str, Any]) -> dict[str, Any]:
    validate_evidence(tauri, "tauri.json")
    validate_evidence(gpui, "gpui.json")
    if tauri["framework"] != "tauri" or gpui["framework"] != "gpui":
        raise EvidenceError("candidate files must be ordered as tauri then gpui")
    for field in ("component_version",):
        if tauri[field] != gpui[field]:
            raise EvidenceError(f"candidate {field} values differ")
    for field in ("path", "sha256", "projection_sha256"):
        if tauri["fixture"][field] != gpui["fixture"][field]:
            raise EvidenceError(f"candidate fixture.{field} values differ")

    gpui_blockers = _blockers(gpui)
    selected = "tauri" if gpui_blockers else "gpui"
    return {
        "schema_version": 1,
        "component_version": tauri["component_version"],
        "selected_framework": selected,
        "decision_rule": "Any non-pass GPUI measurement or hard gate selects Tauri.",
        "gpui_blockers": gpui_blockers,
        "tauri_limitations": _blockers(tauri),
        "evidence": {"tauri": "tauri.json", "gpui": "gpui.json"},
    }


def render_markdown(decision: dict[str, Any]) -> str:
    blockers = "\n".join(f"- `{blocker}`" for blocker in decision["gpui_blockers"])
    limitations = "\n".join(f"- `{item}`" for item in decision["tauri_limitations"])
    return (
        "# GUI Framework Gate Decision Evidence\n\n"
        f"Selected framework: **{decision['selected_framework']}**.\n\n"
        "Rule: any non-pass GPUI measurement or hard gate selects Tauri. "
        "An unavailable or partial result is not treated as a pass.\n\n"
        "## GPUI blockers\n\n"
        f"{blockers or '- None.'}\n\n"
        "## Tauri limitations retained for the production gate\n\n"
        f"{limitations or '- None.'}\n\n"
        "Candidate records: [Tauri](tauri.json) and [GPUI](gpui.json).\n"
    )


def update_framework_gate(path: Path, decision: dict[str, Any]) -> None:
    text = path.read_text(encoding="utf-8")
    text = re.sub(r'^status = ".*"$', 'status = "framework-selected"', text, count=1, flags=re.M)
    text = re.sub(
        r'^selected_framework = ".*"$',
        f'selected_framework = "{decision["selected_framework"]}"',
        text,
        count=1,
        flags=re.M,
    )
    text = re.sub(r"\n\[task4\]\n.*?(?=\n\[|\Z)", "", text, flags=re.S)
    text = text.rstrip() + (
        "\n\n[task4]\n"
        f'selected_framework = "{decision["selected_framework"]}"\n'
        'decision_evidence = "apps/gui/evidence/framework-gate/decision.md"\n'
        'tauri_evidence = "apps/gui/evidence/framework-gate/tauri.json"\n'
        'gpui_evidence = "apps/gui/evidence/framework-gate/gpui.json"\n'
        f'gpui_blocker_count = {len(decision["gpui_blockers"])}\n'
        'production_bootstrap_deferred_to_task5 = true\n'
    )
    path.write_text(text, encoding="utf-8")


def load_evidence(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot load {path}: {error}") from error


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(f"usage: {argv[0]} <tauri.json> <gpui.json>", file=sys.stderr)
        return 2
    try:
        tauri_path = Path(argv[1]).resolve()
        gpui_path = Path(argv[2]).resolve()
        decision = compare_evidence(load_evidence(tauri_path), load_evidence(gpui_path))
        repo_root = Path(__file__).resolve().parents[3]
        evidence_dir = repo_root / "apps/gui/evidence/framework-gate"
        evidence_dir.mkdir(parents=True, exist_ok=True)
        (evidence_dir / "decision.md").write_text(render_markdown(decision), encoding="utf-8")
        update_framework_gate(repo_root / "apps/gui/framework-gate.toml", decision)
    except EvidenceError as error:
        print(f"framework gate evidence error: {error}", file=sys.stderr)
        return 1
    print(json.dumps(decision, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
