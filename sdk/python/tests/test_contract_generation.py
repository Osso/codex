from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_generated_files_are_up_to_date():
    # Regenerate contract artifacts via single maintenance entrypoint.
    env = os.environ.copy()
    python_bin = str(Path(sys.executable).parent)
    env["PATH"] = f"{python_bin}{os.pathsep}{env.get('PATH', '')}"

    subprocess.run(
        [sys.executable, "scripts/update_sdk_artifacts.py", "--types-only"],
        cwd=ROOT,
        check=True,
        env=env,
    )

    # Ensure no diff in generated targets after regeneration.
    diff = subprocess.run(
        [
            "git",
            "diff",
            "--exit-code",
            r"-I^#   timestamp:",
            "--",
            "src/codex_app_server/generated/schema_types.py",
            "src/codex_app_server/generated/protocol_types.py",
            "src/codex_app_server/generated/codex_event_types.py",
            "src/codex_app_server/generated/notification_registry.py",
            "src/codex_app_server/generated/v2_all",
            "src/codex_app_server/public_api.py",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert (
        diff.returncode == 0
    ), f"Generated files drifted:\n{diff.stdout}\n{diff.stderr}"
