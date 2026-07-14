from __future__ import annotations

import copy
import errno
import hashlib
import io
import importlib.machinery
import importlib.util
import os
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from types import ModuleType
from unittest import mock


PROJECT_ROOT = Path(__file__).resolve().parent.parent
PINNED_FIXTURE_SHA256 = (
    "6d6e4246fcdfe0c19f19f696e8c64ed1432c58665718b28269ca61a8a69cc735"
)
EXPECTED_ARCHIVE_DIRECTORIES = ("docs", "fixtures", "fixtures/demo", "policies")
EXPECTED_ARCHIVE_FILES = (
    ("JUDGE_TEST.md", 0o644),
    ("LICENSE", 0o644),
    ("NOTICE", 0o644),
    ("README.md", 0o644),
    ("SECURITY.md", 0o644),
    ("STATUS.md", 0o644),
    ("agentveil", 0o755),
    ("docs/ARCHITECTURE.md", 0o644),
    ("docs/DEMO.md", 0o644),
    ("docs/DONOR_ATTRIBUTION.md", 0o644),
    ("docs/INSTALL.md", 0o644),
    ("docs/JUDGE_TEST.md", 0o644),
    ("docs/LIMITATIONS.md", 0o644),
    ("docs/PAYLOAD_MAP.md", 0o644),
    ("docs/PRIVACY.md", 0o644),
    ("docs/RISK_REGISTER.md", 0o644),
    ("docs/SECURITY_INVARIANTS.md", 0o644),
    ("docs/THREAT_MODEL.md", 0o644),
    ("fixtures/demo/synthetic-context.txt", 0o644),
    ("policies/default.yaml", 0o644),
    ("policies/demo.yaml", 0o644),
    ("policies/strict.yaml", 0o644),
)


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
PACKAGE_RELEASE = load_script("agentveil_package_release_test_target", "package-release")


class ReleaseWorkflowPackagingTests(unittest.TestCase):
    def test_package_release_builds_and_inspects_exact_archive(self) -> None:
        workflow = (PROJECT_ROOT / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("scripts/package-release", workflow)
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary = Path(temporary_directory)
            binary = temporary / "agentveil-test-binary"
            binary.write_bytes(b"#!/bin/sh\nexit 0\n")
            binary.chmod(0o700)
            binary_alias = temporary / "agentveil-test-binary-alias"
            os.link(binary, binary_alias)
            self.assertEqual(binary.stat().st_nlink, 2)
            protected_source = temporary / "protected-source"
            protected_source.write_bytes(b"synthetic source\n")
            os.link(protected_source, temporary / "protected-source-alias")
            with self.assertRaisesRegex(
                PACKAGE_RELEASE.PackageError,
                "^source_not_exclusive_regular_file$",
            ):
                PACKAGE_RELEASE.sha256_regular(protected_source)
            output = temporary / "dist"
            tag = "v0.1.4"
            platform = "macos-arm64"
            package = f"agentveil-{tag}-{platform}"
            archive = output / f"{package}.tar.gz"
            checksum = output / f"{package}.tar.gz.sha256"

            completed = subprocess.run(
                [
                    str(PROJECT_ROOT / "scripts" / "package-release"),
                    "--tag",
                    tag,
                    "--platform",
                    platform,
                    "--binary",
                    str(binary),
                    "--output-dir",
                    str(output),
                ],
                cwd=PROJECT_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(completed.stderr, "")
            self.assertEqual(
                sorted(path.name for path in output.iterdir()),
                sorted((archive.name, checksum.name)),
            )
            self.assertEqual(stat.S_IMODE(archive.stat().st_mode), 0o644)
            self.assertEqual(stat.S_IMODE(checksum.stat().st_mode), 0o644)

            with tarfile.open(archive, "r:gz") as release_archive:
                members = release_archive.getmembers()
                actual_manifest = tuple(
                    (
                        member.name.rstrip("/"),
                        "directory" if member.isdir() else "file",
                        stat.S_IMODE(member.mode),
                    )
                    for member in members
                )
                self.assertFalse(any(member.issym() or member.islnk() for member in members))
                self.assertTrue(all(member.isdir() or member.isreg() for member in members))
                expected_manifest = tuple(
                    sorted(
                        [(package, "directory", 0o755)]
                        + [
                            (f"{package}/{directory}", "directory", 0o755)
                            for directory in EXPECTED_ARCHIVE_DIRECTORIES
                        ]
                        + [
                            (f"{package}/{path}", "file", mode)
                            for path, mode in EXPECTED_ARCHIVE_FILES
                        ]
                    )
                )
                self.assertEqual(actual_manifest, expected_manifest)
                fixture = release_archive.extractfile(
                    f"{package}/{PACKAGE_RELEASE.FIXTURE_PATH}"
                )
                if fixture is None:
                    self.fail("packaged fixture is missing")
                self.assertEqual(
                    hashlib.sha256(fixture.read()).hexdigest(),
                    PINNED_FIXTURE_SHA256,
                )
                self.assertEqual(PACKAGE_RELEASE.FIXTURE_SHA256, PINNED_FIXTURE_SHA256)
                packaged_binary = release_archive.extractfile(f"{package}/agentveil")
                if packaged_binary is None:
                    self.fail("packaged binary is missing")
                self.assertEqual(packaged_binary.read(), binary.read_bytes())

            archive_digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            self.assertEqual(
                checksum.read_text(encoding="ascii"),
                f"{archive_digest}  {archive.name}\n",
            )
            third_binary_alias = temporary / "agentveil-test-binary-third-link"
            os.link(binary, third_binary_alias)
            self.assertEqual(binary.stat().st_nlink, 3)
            with self.assertRaisesRegex(
                PACKAGE_RELEASE.PackageError,
                "^source_not_exclusive_regular_file$",
            ):
                PACKAGE_RELEASE.package_release(
                    tag=tag,
                    platform=platform,
                    binary=binary,
                    output_dir=temporary / "three-link-output",
                )
            third_binary_alias.unlink()
            self.assertEqual(binary.stat().st_nlink, 2)
            extra_archive = temporary / "extra-member.tar.gz"
            with tarfile.open(archive, "r:gz") as source_archive:
                with tarfile.open(
                    extra_archive, "w:gz", format=tarfile.USTAR_FORMAT
                ) as destination_archive:
                    for member in source_archive.getmembers():
                        stream = (
                            source_archive.extractfile(member)
                            if member.isreg()
                            else None
                        )
                        destination_archive.addfile(copy.copy(member), stream)
                    extra = tarfile.TarInfo(f"{package}/extra")
                    extra.mode = 0o644
                    extra.size = 5
                    extra.mtime = 0
                    destination_archive.addfile(extra, io.BytesIO(b"extra"))
            with self.assertRaisesRegex(
                PACKAGE_RELEASE.PackageError, "^archive_manifest_mismatch$"
            ):
                PACKAGE_RELEASE.validate_archive(extra_archive, package)

            link_archive = temporary / "link-member.tar.gz"
            with tarfile.open(link_archive, "w:gz") as release_archive:
                link = tarfile.TarInfo(f"{package}/link")
                link.type = tarfile.SYMTYPE
                link.linkname = "agentveil"
                link.mode = 0o777
                link.mtime = 0
                release_archive.addfile(link)
            with self.assertRaisesRegex(
                PACKAGE_RELEASE.PackageError,
                "^archive_unsupported_member_type$",
            ):
                PACKAGE_RELEASE.validate_archive(link_archive, package)

            special_mode_archive = temporary / "special-mode.tar.gz"
            with tarfile.open(archive, "r:gz") as source_archive:
                with tarfile.open(
                    special_mode_archive, "w:gz", format=tarfile.USTAR_FORMAT
                ) as destination_archive:
                    for member in source_archive.getmembers():
                        cloned = copy.copy(member)
                        if member.name.rstrip("/") == f"{package}/agentveil":
                            cloned.mode = 0o4755
                        stream = (
                            source_archive.extractfile(member)
                            if member.isreg()
                            else None
                        )
                        destination_archive.addfile(cloned, stream)
            with self.assertRaisesRegex(
                PACKAGE_RELEASE.PackageError, "^archive_manifest_mismatch$"
            ):
                PACKAGE_RELEASE.validate_archive(special_mode_archive, package)

            bad_repository = temporary / "bad-repository"
            bad_fixture = bad_repository / "fixtures" / "demo" / "synthetic-context.txt"
            bad_fixture.parent.mkdir(parents=True)
            bad_fixture.write_bytes(b"synthetic mismatch\n")
            with mock.patch.object(PACKAGE_RELEASE, "REPO_ROOT", bad_repository):
                with self.assertRaisesRegex(
                    PACKAGE_RELEASE.PackageError, "^fixture_hash_mismatch$"
                ):
                    PACKAGE_RELEASE.package_release(
                        tag=tag,
                        platform=platform,
                        binary=binary,
                        output_dir=temporary / "bad-output",
                    )
            with self.assertRaisesRegex(
                PACKAGE_RELEASE.PackageError, "^invalid_tag$"
            ):
                PACKAGE_RELEASE.package_release(
                    tag=f"v{'9' * 140}.1.1",
                    platform=platform,
                    binary=binary,
                    output_dir=temporary / "long-tag-output",
                )


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
            with mock.patch.object(LEAK_SCANNER, "REPO_ROOT", repository):
                with mock.patch.object(LEAK_SCANNER, "REPORT_ROOT", report_root):
                    with self.assertRaisesRegex(ValueError, f"^{expected_message}$"):
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

            with mock.patch.object(BENCHMARK, "REPO_ROOT", repository):
                with mock.patch.object(
                    BENCHMARK.os, "open", side_effect=tracking_open
                ):
                    with mock.patch.object(
                        BENCHMARK.os, "fstat", side_effect=failing_fstat
                    ):
                        with self.assertRaises(OSError):
                            BENCHMARK.open_report_directory()

            self.assertEqual(len(opened_generated), 1)
            with self.assertRaises(OSError) as closed_error:
                os.fstat(opened_generated[0])
            self.assertEqual(closed_error.exception.errno, errno.EBADF)


if __name__ == "__main__":
    unittest.main()
