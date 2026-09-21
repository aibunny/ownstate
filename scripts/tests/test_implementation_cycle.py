"""Exercise loop failure and evidence semantics without running arbitrary commands."""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "implementation_cycle.py"
SPEC = importlib.util.spec_from_file_location("implementation_cycle", SCRIPT)
cycle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cycle)


class CycleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / "Cargo.toml").write_text("[workspace]\n")
        (self.root / "openspec/changes/example-change").mkdir(parents=True)
        (self.root / ".opsx-current").write_text("example-change")
        self.which = patch.object(cycle.shutil, "which", return_value="/test/cargo")
        self.which.start()
        self.addCleanup(self.which.stop)

    def receipt(self):
        return json.loads((self.root / "target/implementation-loop/latest.json").read_text())

    def test_first_failed_gate_stops_and_never_claims_code_failure(self):
        with patch.object(cycle.subprocess, "run", return_value=subprocess.CompletedProcess([], 1)) as run:
            self.assertEqual(cycle.run_cycle(self.root, "baseline"), 1)
        run.assert_called_once_with(cycle.GATES[0], cwd=self.root, check=False)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "failed")
        self.assertIn("external block", receipt["next_action"])
        self.assertFalse(receipt["batch_complete"])
        self.assertEqual(len(receipt["gates"]), 1)

    def test_checks_passed_still_requires_review_and_fixed_commands(self):
        with patch.object(cycle.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as run:
            self.assertEqual(cycle.run_cycle(self.root, "verify"), 0)
        self.assertEqual([call.args[0] for call in run.call_args_list], list(cycle.GATES))
        self.assertEqual(cycle.GATES[2], ("cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"))
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "checks_passed")
        self.assertEqual(receipt["independent_review"], "pending")
        self.assertEqual(receipt["documentation_review"], "pending")
        self.assertFalse(receipt["batch_complete"])
        self.assertIsNone(receipt["baseline_source_manifest"])
        self.assertIsNone(receipt["changed_paths"])

    def test_missing_cargo_preserves_exact_safe_block_reason(self):
        with patch.object(cycle.shutil, "which", return_value=None), patch.object(cycle.subprocess, "run") as run:
            self.assertEqual(cycle.run_cycle(self.root, "baseline"), 2)
        run.assert_not_called()
        self.assertEqual(self.receipt()["reason"], "Cargo executable is unavailable")
        self.assertEqual(self.receipt()["status"], "blocked")

    def test_verify_rejects_missing_or_invalid_change(self):
        for change in ("missing", "../../outside", "UPPER"):
            with self.subTest(change=change), patch.object(cycle.subprocess, "run") as run:
                self.assertEqual(cycle.run_cycle(self.root, "verify", change), 2)
                run.assert_not_called()

    def test_interrupt_is_not_a_passed_gate(self):
        with patch.object(cycle.subprocess, "run", side_effect=KeyboardInterrupt):
            self.assertEqual(cycle.run_cycle(self.root, "baseline"), 130)
        receipt = self.receipt()
        self.assertEqual(receipt["status"], "interrupted")
        self.assertIsNone(receipt["gates"][0]["exit_code"])

    def test_symlinked_receipt_directory_and_file_are_rejected(self):
        outside = self.root / "outside"
        outside.mkdir()
        (self.root / "target").symlink_to(outside, target_is_directory=True)
        with patch.object(cycle.subprocess, "run") as run:
            self.assertEqual(cycle.run_cycle(self.root, "baseline"), 2)
            run.assert_not_called()
        self.assertEqual(list(outside.iterdir()), [])
        (self.root / "target").unlink()
        (self.root / "target/implementation-loop").mkdir(parents=True)
        secret = outside / "keep.json"
        secret.write_text("preserve")
        (self.root / "target/implementation-loop/latest.json").symlink_to(secret)
        self.assertEqual(cycle.run_cycle(self.root, "baseline"), 2)
        self.assertEqual(secret.read_text(), "preserve")

    def test_manifest_excludes_secrets_caches_symlinks_and_workflow_state(self):
        (self.root / ".env").write_text("SECRET=never-read")
        (self.root / ".env.example").write_text("EXAMPLE=value")
        (self.root / ".dockerignore").write_text(".env\ntarget\n")
        (self.root / "STATE.md").write_text("workflow metadata")
        (self.root / "docs").mkdir()
        (self.root / "docs/.env.local").write_text("SECRET=never-read")
        (self.root / "docs/symlink.md").symlink_to(self.root / ".env")
        (self.root / "docs/source.md").write_text("documentation")
        manifest = cycle.source_manifest(self.root)
        self.assertEqual(set(manifest), {"Cargo.toml", ".env.example", ".dockerignore", "docs/source.md"})

    def test_baseline_manifest_survives_verify_and_records_source_delta(self):
        with patch.object(cycle.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)):
            self.assertEqual(cycle.run_cycle(self.root, "baseline"), 0)
            original = self.receipt()["source_manifest"]
            (self.root / "Cargo.toml").write_text("[workspace]\nresolver = '3'\n")
            self.assertEqual(cycle.run_cycle(self.root, "verify"), 0)
            self.assertEqual(cycle.run_cycle(self.root, "verify"), 0)
        receipt = self.receipt()
        self.assertEqual(receipt["baseline_source_manifest"], original)
        self.assertEqual(receipt["changed_paths"], ["Cargo.toml"])

    def test_source_mutation_during_checks_invalidates_pass(self):
        def mutate(command, **kwargs):
            (self.root / "Cargo.toml").write_text("changed")
            return subprocess.CompletedProcess(command, 0)
        with patch.object(cycle.subprocess, "run", side_effect=mutate):
            self.assertEqual(cycle.run_cycle(self.root, "verify"), 1)
        self.assertFalse(self.receipt()["source_manifest_stable"])
        self.assertEqual(self.receipt()["status"], "failed")

    def test_receipt_write_failure_reports_blocked_without_false_console_pass(self):
        with patch.object(cycle.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)), \
                patch.object(cycle, "write_receipt", side_effect=PermissionError), \
                patch("builtins.print") as output:
            self.assertEqual(cycle.run_cycle(self.root, "baseline"), 2)
        self.assertIn("Check status: blocked", output.call_args.args[0])


if __name__ == "__main__":
    unittest.main()
