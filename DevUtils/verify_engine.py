#!/usr/bin/env python3
"""Verify that every Quasar dependency pins the same repository tag and lock commit."""
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/HungLo2020/Quasar"
TAG = "v0.2.5"
LOCK_PREFIX = f"git+{REPOSITORY}?tag={TAG}#"


def main() -> None:
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    direct = {
        name: dependency
        for name, dependency in manifest["dependencies"].items()
        if name.startswith("quasar-")
    }
    assert len(direct) == 6, f"expected six direct Quasar crates, found {sorted(direct)}"
    for name, dependency in direct.items():
        assert dependency.get("git") == REPOSITORY, f"{name} does not use the Quasar Git repository"
        assert dependency.get("tag") == TAG, f"{name} does not pin {TAG}"
        assert "path" not in dependency, f"{name} uses a local path dependency"

    locked = tomllib.loads((ROOT / "Cargo.lock").read_text())
    engine = [
        package
        for package in locked["package"]
        if package["name"].startswith("quasar-") or package["name"] == "vulkanic-gal"
    ]
    assert len(engine) == 8, f"expected seven Quasar crates plus VulkanicGAL, found {len(engine)}"
    commits = {package["source"] for package in engine}
    assert len(commits) == 1, f"engine crates resolve to different commits: {sorted(commits)}"
    assert all(package["source"] == next(iter(commits)) for package in engine)
    assert next(iter(commits)).startswith(LOCK_PREFIX), commits
    assert all(package["version"] == "0.2.5" for package in engine)
    print(f"All Quasar dependencies resolve to {TAG} from Git.")


if __name__ == "__main__":
    main()
