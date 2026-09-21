#!/usr/bin/env python3
"""Run fixed repository checks and record their evidence, never batch completion."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from datetime import datetime, timezone


GATES = (
    ("cargo", "fmt", "--check"),
    ("cargo", "check", "--workspace"),
    ("cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"),
    ("cargo", "test", "--workspace"),
)
SOURCE_DIRS = ("apps", "crates", "migrations", "docs", "scripts", ".github")
SOURCE_SUFFIXES = {".rs", ".toml", ".lock", ".sql", ".md", ".sh", ".py", ".yml", ".yaml", ".json"}
EXCLUDED_DIRS = {"target", ".git", "__pycache__", "node_modules", ".fastembed_cache", "attachments"}
ROOT_FILES = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "docker-compose.yml",
    "README.md", "ARCHITECTURE.md", "REQUIREMENTS.md",
    "CODEX_IMPLEMENTATION_PROMPT.md", "BUNDLE.md", ".env.example",
    "opsx", "opsx:propose", "opsx:apply", "opsx:archive", "Dockerfile", ".dockerignore",
}


class Blocked(Exception):
    pass


def utc_now():
    return datetime.now(timezone.utc).isoformat()


def source_manifest(root):
    """Hash allowed source/config files; never read secret env files or symlinks."""
    def traversal_error(error):
        raise error

    paths = [root / name for name in ROOT_FILES]
    for name in SOURCE_DIRS:
        directory = root / name
        if directory.is_symlink():
            continue
        if not directory.exists():
            continue
        for current, directories, files in os.walk(directory, followlinks=False, onerror=traversal_error):
            directories[:] = sorted(
                name for name in directories
                if name not in EXCLUDED_DIRS and not (Path(current) / name).is_symlink()
            )
            for name in sorted(files):
                path = Path(current) / name
                if not name.startswith(".env") and path.suffix in SOURCE_SUFFIXES:
                    paths.append(path)
    return {
        path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(paths)
        if path.is_file() and not path.is_symlink()
    }


def receipt_path(root):
    path = root / "target" / "implementation-loop" / "latest.json"
    for candidate in (root / "target", path.parent, path):
        if candidate.is_symlink():
            raise Blocked("Receipt directories and files must not be symlinks")
    if path.parent.exists() and not path.parent.is_dir():
        raise Blocked("Receipt directory is not a directory")
    if path.exists() and not path.is_file():
        raise Blocked("Receipt target is not a regular file")
    return path


def write_receipt(root, receipt):
    path = receipt_path(root)
    path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path(root)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix=".receipt-", suffix=".json", delete=False) as file:
            temporary = Path(file.name)
            json.dump(receipt, file, indent=2, sort_keys=True)
            file.write("\n")
            file.flush()
            os.fsync(file.fileno())
        receipt_path(root)
        os.replace(temporary, path)
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


def resolve_change(root, phase, change_id):
    if change_id is None and phase == "verify":
        current = root / ".opsx-current"
        if current.is_symlink():
            raise Blocked("Change selector must not be a symlink")
        if current.is_file():
            change_id = current.read_text(encoding="utf-8").strip()
    if change_id is not None and re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", change_id) is None:
        raise Blocked("Change id must be a lowercase slug")
    if phase == "verify":
        if not change_id:
            raise Blocked("Verify requires an opsx change id")
        change = root / "openspec" / "changes" / change_id
        if any(path.is_symlink() for path in (root / "openspec", change.parent, change)):
            raise Blocked("OpenSpec change path must not be a symlink")
        if not change.is_dir():
            raise Blocked("Verify requires an existing OpenSpec change")
    return change_id


def run_cycle(root, phase, change_id=None):
    root = Path(root).resolve()
    receipt = {
        "schema_version": 1, "phase": phase, "change_id": None,
        "started_at": utc_now(), "finished_at": None, "status": "blocked",
        "gates": [], "source_manifest": {}, "baseline_source_manifest": None,
        "changed_paths": None, "source_manifest_stable": None,
        "independent_review": "pending", "documentation_review": "pending",
        "batch_complete": False,
        "next_action": "Run checks, then obtain independent and documentation review",
    }
    exit_code = 2
    before = None
    try:
        path = receipt_path(root)
        if phase not in {"baseline", "verify"}:
            raise Blocked("Unknown phase")
        receipt["change_id"] = resolve_change(root, phase, change_id)
        if path.is_file():
            try:
                previous = json.loads(path.read_text(encoding="utf-8"))
                baseline = previous.get("baseline_source_manifest")
                if isinstance(baseline, dict) and all(
                    isinstance(key, str) and isinstance(value, str) for key, value in baseline.items()
                ):
                    receipt["baseline_source_manifest"] = baseline
            except (ValueError, AttributeError):
                pass
        before = source_manifest(root)
        receipt["source_manifest"] = before
        if phase == "baseline":
            receipt["baseline_source_manifest"] = before
        baseline = receipt["baseline_source_manifest"]
        if baseline is not None:
            receipt["changed_paths"] = sorted(
                key for key in baseline.keys() | before.keys() if baseline.get(key) != before.get(key)
            )
        if shutil.which("cargo") is None:
            raise Blocked("Cargo executable is unavailable")
        for command in GATES:
            print("Running " + " ".join(command), flush=True)
            gate = {"command": list(command), "exit_code": None}
            receipt["gates"].append(gate)
            result = subprocess.run(command, cwd=root, check=False)
            gate["exit_code"] = result.returncode
            if result.returncode != 0:
                receipt["status"] = "failed"
                receipt["reason"] = "A fixed check failed; inspect its console output"
                receipt["next_action"] = "Triage the failed verification gate; confirm code failure versus external block before retrying"
                exit_code = 1
                break
        else:
            receipt["status"] = "checks_passed"
            receipt["next_action"] = "Resolve independent checker findings and confirm documentation before recording batch completion"
            exit_code = 0
    except KeyboardInterrupt:
        receipt["status"] = "interrupted"
        receipt["reason"] = "Execution interrupted; unfinished checks have no passing evidence"
        receipt["next_action"] = "Confirm interrupted processes stopped, then rerun verification on a stable snapshot"
        exit_code = 130
    except Blocked as error:
        receipt["status"] = "blocked"
        receipt["reason"] = str(error)
        receipt["next_action"] = "Record the blocker and next viable route in STATE.md; resolve it before retrying"
        exit_code = 2
    except (FileNotFoundError, PermissionError):
        receipt["status"] = "blocked"
        receipt["reason"] = "Required tool, change metadata, filesystem access, or safe receipt path unavailable"
        receipt["next_action"] = "Record a sanitized exact diagnostic and next viable route in STATE.md; resolve the confirmed block before retrying"
        exit_code = 2
    except OSError:
        receipt["status"] = "blocked"
        receipt["reason"] = "Operating system prevented check execution or evidence collection"
        receipt["next_action"] = "Record a sanitized exact diagnostic and next viable route in STATE.md; resolve the confirmed block before retrying"
        exit_code = 2
    finally:
        if before is not None:
            try:
                receipt["source_manifest_stable"] = source_manifest(root) == before
                if not receipt["source_manifest_stable"] and receipt["status"] == "checks_passed":
                    receipt["status"] = "failed"
                    receipt["reason"] = "Source changed during checks; verify a stable source snapshot"
                    receipt["next_action"] = "Finish source changes and rerun verification on a stable snapshot"
                    exit_code = 1
            except OSError:
                receipt["source_manifest_stable"] = False
                receipt["status"] = "blocked"
                receipt["reason"] = "Unable to confirm the tested source snapshot"
                exit_code = 2
        receipt["finished_at"] = utc_now()
        try:
            write_receipt(root, receipt)
        except Blocked as error:
            receipt["status"] = "blocked"
            receipt["reason"] = str(error)
            print("Blocked: " + receipt["reason"], flush=True)
            exit_code = 2
        except OSError:
            receipt["status"] = "blocked"
            receipt["reason"] = "Receipt write was prevented by the operating system"
            print("Blocked: cannot safely write the receipt", flush=True)
            exit_code = 2
    print("Check status: " + receipt["status"] + "; independent and documentation review remain pending", flush=True)
    return exit_code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", required=True, choices=("baseline", "verify"))
    parser.add_argument("--change-id")
    args = parser.parse_args()
    return run_cycle(Path(__file__).resolve().parent.parent, args.phase, args.change_id)


if __name__ == "__main__":
    raise SystemExit(main())
