import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


TOOLS_ROOT = Path(__file__).resolve().parents[1]
COMPARE_PATH = TOOLS_ROOT / "compare-framework-gate.py"
RUNNER_PATH = TOOLS_ROOT / "run-framework-gate.sh"
LAUNCH_CHECK_PATH = TOOLS_ROOT / "check-process-survival.py"
FIXTURE_CHECK_PATH = TOOLS_ROOT / "verify-framework-fixture.py"
CANONICAL_FIXTURE = (
    TOOLS_ROOT.parents[2]
    / "crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json"
)


class FrameworkGateTests(unittest.TestCase):
    def load_comparator(self):
        self.assertTrue(COMPARE_PATH.is_file(), "comparator implementation is missing")
        spec = importlib.util.spec_from_file_location("framework_gate_comparator", COMPARE_PATH)
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def passing_evidence(self, module, framework: str) -> dict:
        measurements = {
            name: {
                "status": "pass",
                "value": (
                    threshold["value"] - 1
                    if threshold["operator"] == "lt"
                    else threshold["value"]
                ),
                "unit": threshold["unit"],
                "threshold": threshold,
                "evidence": ["command:test"],
            }
            for name, threshold in module.REQUIRED_MEASUREMENTS.items()
        }
        hard_gates = {
            name: {
                "status": "pass",
                "summary": "verified",
                "evidence": ["command:test"],
            }
            for name in module.REQUIRED_HARD_GATES
        }
        return {
            "schema_version": 1,
            "framework": framework,
            "component_version": "0.1.0-alpha.1",
            "fixture": {
                "path": "crates/types/tests/fixtures/frontend-contract-v1/d1-vertical-slice.json",
                "sha256": "a" * 64,
                "projection_sha256": "7dd8faf04cca9f3013198e25823894eae91c2869e27087aa1eb0a34890cdf804",
            },
            "host": {"os": "test", "arch": "test"},
            "commands": [
                {
                    "name": "test",
                    "command": "test",
                    "status": "pass",
                    "exit_code": 0,
                    "output_tail": [],
                }
            ],
            "measurements": measurements,
            "hard_gates": hard_gates,
        }

    def test_validator_rejects_pass_without_reproducible_evidence(self):
        module = self.load_comparator()
        candidate = self.passing_evidence(module, "gpui")
        candidate["hard_gates"]["screen_reader"]["evidence"] = []

        with self.assertRaisesRegex(module.EvidenceError, "pass requires evidence"):
            module.validate_evidence(candidate, "gpui.json")

    def test_validator_rejects_unknown_command_evidence(self):
        module = self.load_comparator()
        candidate = self.passing_evidence(module, "gpui")
        candidate["hard_gates"]["screen_reader"]["evidence"] = ["command:not-recorded"]

        with self.assertRaisesRegex(module.EvidenceError, "unknown command evidence"):
            module.validate_evidence(candidate, "gpui.json")

    def test_validator_rejects_pass_records_backed_by_a_failed_command(self):
        module = self.load_comparator()

        for record_kind, record_name in (
            ("hard_gates", "screen_reader"),
            ("measurements", "ordered_event_count"),
        ):
            with self.subTest(record_kind=record_kind):
                candidate = self.passing_evidence(module, "gpui")
                candidate["commands"][0].update(status="fail", exit_code=1)
                candidate[record_kind][record_name]["evidence"] = ["command:test"]

                with self.assertRaisesRegex(module.EvidenceError, "failed command evidence"):
                    module.validate_evidence(candidate, "gpui.json")

    def test_any_unavailable_gpui_hard_gate_selects_tauri(self):
        module = self.load_comparator()
        tauri = self.passing_evidence(module, "tauri")
        gpui = self.passing_evidence(module, "gpui")
        gpui["hard_gates"]["screen_reader"] = {
            "status": "unavailable",
            "summary": "No screen-reader run was captured",
            "evidence": [],
        }

        decision = module.compare_evidence(tauri, gpui)

        self.assertEqual(decision["selected_framework"], "tauri")
        self.assertIn("hard_gates.screen_reader:unavailable", decision["gpui_blockers"])

    def test_equal_d1_slice_is_a_required_hard_gate(self):
        module = self.load_comparator()

        self.assertIn("d1_slice_parity", module.REQUIRED_HARD_GATES)

    def test_non_pass_d1_slice_blocks_gpui_and_missing_d1_is_invalid(self):
        module = self.load_comparator()
        tauri = self.passing_evidence(module, "tauri")

        for status in ("partial", "fail", "unavailable"):
            with self.subTest(status=status):
                gpui = self.passing_evidence(module, "gpui")
                gpui["hard_gates"]["d1_slice_parity"] = {
                    "status": status,
                    "summary": "D1 parity is not proven",
                    "evidence": ["command:test"] if status != "unavailable" else [],
                }
                decision = module.compare_evidence(tauri, gpui)
                self.assertEqual(decision["selected_framework"], "tauri")
                self.assertIn(
                    f"hard_gates.d1_slice_parity:{status}", decision["gpui_blockers"]
                )

        missing = self.passing_evidence(module, "gpui")
        del missing["hard_gates"]["d1_slice_parity"]
        with self.assertRaises(module.EvidenceError):
            module.validate_evidence(missing, "gpui.json")

    def test_missing_gpui_timing_measurement_selects_tauri(self):
        module = self.load_comparator()
        tauri = self.passing_evidence(module, "tauri")
        gpui = self.passing_evidence(module, "gpui")
        gpui["measurements"]["frame_work_p95_ms"] = {
            "status": "unavailable",
            "value": None,
            "unit": "ms",
            "threshold": module.REQUIRED_MEASUREMENTS["frame_work_p95_ms"],
            "evidence": [],
        }

        decision = module.compare_evidence(tauri, gpui)

        self.assertEqual(decision["selected_framework"], "tauri")
        self.assertIn("measurements.frame_work_p95_ms:unavailable", decision["gpui_blockers"])

    def test_runner_rejects_an_unknown_framework_before_collecting_evidence(self):
        self.assertTrue(RUNNER_PATH.is_file(), "gate runner implementation is missing")
        with tempfile.TemporaryDirectory() as temporary_directory:
            fixture = Path(temporary_directory) / "fixture.json"
            fixture.write_text(json.dumps({"schema_version": 1}), encoding="utf-8")
            result = subprocess.run(
                [str(RUNNER_PATH), "unknown", str(fixture)],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(result.returncode, 2)
        self.assertIn("framework must be tauri or gpui", result.stderr)

    def test_launch_check_rejects_a_clean_exit_before_the_full_interval(self):
        self.assertTrue(LAUNCH_CHECK_PATH.is_file(), "launch survival checker is missing")
        result = subprocess.run(
            [str(LAUNCH_CHECK_PATH), "0.2", "/usr/bin/true"],
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("before the required", result.stderr)

    def test_launch_check_accepts_a_process_alive_for_the_full_interval(self):
        self.assertTrue(LAUNCH_CHECK_PATH.is_file(), "launch survival checker is missing")
        result = subprocess.run(
            [str(LAUNCH_CHECK_PATH), "0.2", "/bin/sleep", "1"],
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("remained alive for the required", result.stdout)

    def test_fixture_check_binds_tests_to_the_canonical_fixture_identity(self):
        self.assertTrue(FIXTURE_CHECK_PATH.is_file(), "fixture identity checker is missing")
        canonical = subprocess.run(
            [str(FIXTURE_CHECK_PATH), str(CANONICAL_FIXTURE), str(CANONICAL_FIXTURE)],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(canonical.returncode, 0, canonical.stderr)

        with tempfile.TemporaryDirectory() as temporary_directory:
            substituted = Path(temporary_directory) / "fixture.json"
            payload = json.loads(CANONICAL_FIXTURE.read_text(encoding="utf-8"))
            payload["substitution_probe"] = True
            substituted.write_text(json.dumps(payload), encoding="utf-8")
            result = subprocess.run(
                [str(FIXTURE_CHECK_PATH), str(substituted), str(CANONICAL_FIXTURE)],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match the canonical fixture", result.stderr)


if __name__ == "__main__":
    unittest.main()
