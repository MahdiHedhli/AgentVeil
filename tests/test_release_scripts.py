from __future__ import annotations

import errno
import importlib.machinery
import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from unittest import mock


PROJECT_ROOT = Path(__file__).resolve().parent.parent


def load_script(module_name: str, script_name: str) -> ModuleType:
    script_path = PROJECT_ROOT / "scripts" / script_name
    loader = importlib.machinery.SourceFileLoader(module_name, str(script_path))
    spec = importlib.util.spec_from_loader(module_name, loader)
    if spec is None or spec.loader is None:
        raise RuntimeError("release script could not be loaded")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


BENCHMARK = load_script("agentveil_benchmark_test_target", "benchmark")
LEAK_SCANNER = load_script("agentveil_leak_scanner_test_target", "verify-no-leak")


class LeakReportDirectoryTests(unittest.TestCase):
    def assert_report_rejected(self, layout: str, expected_message: str) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            repository = Path(temporary_directory)
            reports = repository / "reports"
            reports.mkdir(mode=0o700)
            report_root = reports / "generated"

            if layout == "symlink":
                target = repository / "target"
                target.mkdir(mode=0o700)
                report_root.symlink_to(target, target_is_directory=True)
            elif layout == "not_directory":
                report_root.write_text("placeholder", encoding="utf-8")
            elif layout == "broad_permissions":
                report_root.mkdir(mode=0o700)
                report_root.chmod(0o755)
            else:
                self.fail("unsupported test layout")

            metadata = LEAK_SCANNER.RepositoryMetadata(
                head_commit="0" * 40, clean_worktree=False
            )
            with (
                mock.patch.object(LEAK_SCANNER, "REPO_ROOT", repository),
                mock.patch.object(LEAK_SCANNER, "REPORT_ROOT", report_root),
                self.assertRaisesRegex(ValueError, f"^{expected_message}$"),
            ):
                LEAK_SCANNER.write_report(
                    str(report_root / "leak-scan.json"),
                    "fail",
                    False,
                    metadata,
                    {},
                    0,
                    0,
                    0,
                    [],
                    [],
                )

    def test_report_directory_failures_have_distinct_messages(self) -> None:
        cases = (
            ("symlink", "report directory must not contain symlinks"),
            ("not_directory", "report directory must be a directory"),
            ("broad_permissions", "report directory permissions must be 0700"),
        )
        for layout, expected_message in cases:
            with self.subTest(layout=layout):
                self.assert_report_rejected(layout, expected_message)


class BenchmarkReportDirectoryTests(unittest.TestCase):
    def test_generated_descriptor_closes_when_validation_raises(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            repository = Path(temporary_directory)
            generated = repository / "reports" / "generated"
            generated.mkdir(parents=True, mode=0o700)
            generated.chmod(0o700)

            opened_generated: list[int] = []
            original_open = os.open
            original_fstat = os.fstat

            def tracking_open(path: object, *args: object, **kwargs: object) -> int:
                descriptor = original_open(path, *args, **kwargs)
                if path == "generated":
                    opened_generated.append(descriptor)
                return descriptor

            def failing_fstat(descriptor: int) -> os.stat_result:
                if opened_generated and descriptor == opened_generated[-1]:
                    raise OSError(errno.EIO, "synthetic validation failure")
                return original_fstat(descriptor)

            with (
                mock.patch.object(BENCHMARK, "REPO_ROOT", repository),
                mock.patch.object(BENCHMARK.os, "open", side_effect=tracking_open),
                mock.patch.object(BENCHMARK.os, "fstat", side_effect=failing_fstat),
                self.assertRaises(OSError),
            ):
                BENCHMARK.open_report_directory()

            self.assertEqual(len(opened_generated), 1)
            with self.assertRaises(OSError) as closed_error:
                os.fstat(opened_generated[0])
            self.assertEqual(closed_error.exception.errno, errno.EBADF)


if __name__ == "__main__":
    unittest.main()
