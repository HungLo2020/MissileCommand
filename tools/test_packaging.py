"""Regression checks for archive boundaries that protect Basalt user data."""

from pathlib import Path
import stat
import tempfile
import unittest
import warnings
import zipfile

from package import add_file
from validate_package import validate_archive


class PackageBoundaryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "release.zip"
        self.source = self.root / "source"
        self.source.write_bytes(b"fixture")
        self.make_package()

    def make_package(self) -> None:
        with zipfile.ZipFile(self.archive, "w") as package:
            add_file(package, self.source, "app/bin/missile-command", True)
            add_file(package, self.source, "app/README.md", False)

    def append(self, name: str, mode: int = stat.S_IFREG | 0o644) -> None:
        with zipfile.ZipFile(self.archive, "a") as package:
            entry = zipfile.ZipInfo(name)
            entry.create_system = 3
            entry.external_attr = mode << 16
            with warnings.catch_warnings():
                warnings.simplefilter("ignore", UserWarning)
                package.writestr(entry, b"fixture")

    def test_self_contained_archive_and_executable_permissions(self) -> None:
        validate_archive(self.archive)
        with zipfile.ZipFile(self.archive) as package:
            mode = package.getinfo("app/bin/missile-command").external_attr >> 16
        self.assertTrue(mode & stat.S_IXUSR)

    def test_rejects_paths_that_could_escape_app(self) -> None:
        for name in ("data/save/scores", "../save", "app/../../save", "/app/bad", "app\\..\\save", "app//bad"):
            with self.subTest(name=name):
                self.make_package()
                self.append(name)
                with self.assertRaises(ValueError):
                    validate_archive(self.archive)

    def test_rejects_duplicate_executable(self) -> None:
        self.append("app/bin/missile-command")
        with self.assertRaisesRegex(ValueError, "duplicate"):
            validate_archive(self.archive)

    def test_rejects_symlink(self) -> None:
        self.append("app/link", stat.S_IFLNK | 0o777)
        with self.assertRaisesRegex(ValueError, "symlink"):
            validate_archive(self.archive)

    def test_rejects_file_as_directory(self) -> None:
        self.append("app")
        with self.assertRaisesRegex(ValueError, "directory"):
            validate_archive(self.archive)

    def test_rejects_missing_executable_permission(self) -> None:
        with zipfile.ZipFile(self.archive, "w") as package:
            add_file(package, self.source, "app/bin/missile-command", False)
            add_file(package, self.source, "app/README.md", False)
        with self.assertRaisesRegex(ValueError, "execute bit"):
            validate_archive(self.archive)


if __name__ == "__main__":
    unittest.main()
