#!/usr/bin/env python3
from pathlib import Path
import json
import shutil
import sys

SOURCE = Path.home() / ".local/share/mise/installs/node/24.11.0/lib/node_modules/pi-multi-auth"
TARGET = Path(__file__).resolve().parents[2] / ".pi/packages/pi-multi-auth-aoc"
COPY_NAMES = ["index.ts", "package.json", "README.md", "CHANGELOG.md", "LICENSE", "config.json", "src", "scripts"]
MUTABLE_DIR_NAMES = ["debug"]


def patch_package_json(path: Path) -> None:
    data = json.loads(path.read_text())
    data["name"] = "pi-multi-auth-aoc"
    data["version"] = "0.1.2-aoc.1"
    data["private"] = True
    data["description"] = "AOC-vendored pi-multi-auth package with OpenRouter-aware multi-auth support."
    data["homepage"] = "https://github.com/ceii/agent-ops-cockpit"
    data["repository"] = {"type": "git", "url": "git+https://github.com/ceii/agent-ops-cockpit.git"}
    data["bugs"] = {"url": "https://github.com/ceii/agent-ops-cockpit/issues"}
    data.pop("publishConfig", None)
    data["dependencies"] = {}
    dev = data.get("devDependencies") or {}
    dev.pop("@types/proper-lockfile", None)
    data["devDependencies"] = dev
    path.write_text(json.dumps(data, indent=2) + "\n")


def patch_readme(path: Path) -> None:
    text = path.read_text()
    old = "# pi-multi-auth\n\n[![npm version](https://img.shields.io/npm/v/pi-multi-auth.svg)](https://www.npmjs.com/package/pi-multi-auth) [![GitHub](https://img.shields.io/badge/GitHub-MasuRii%2Fpi--multi--auth-blue)](https://github.com/MasuRii/pi-multi-auth)\n"
    new = "# pi-multi-auth-aoc\n\nVendored AOC-managed fork of `pi-multi-auth`, delivered as a local Pi package under `.pi/packages/pi-multi-auth-aoc`.\n\nAOC-specific changes:\n- native OpenRouter provider discovery/registration in multi-auth\n- Pi built-in provider metadata stays authoritative; `~/.pi/agent/models.json` is treated as additive override input\n- intended for project-local delivery via `.pi/settings.json` package path, not global npm install\n\nOriginal upstream project: [MasuRii/pi-multi-auth](https://github.com/MasuRii/pi-multi-auth)\n"
    if old in text:
        text = text.replace(old, new)
    path.write_text(text)


def main() -> int:
    if not SOURCE.exists():
        print(f"missing source package: {SOURCE}", file=sys.stderr)
        return 1

    if TARGET.exists():
        shutil.rmtree(TARGET)
    TARGET.mkdir(parents=True, exist_ok=True)

    for name in COPY_NAMES:
        src = SOURCE / name
        dst = TARGET / name
        if src.is_dir():
            shutil.copytree(src, dst)
        else:
            shutil.copy2(src, dst)

    patch_package_json(TARGET / "package.json")
    patch_readme(TARGET / "README.md")
    for name in MUTABLE_DIR_NAMES:
        shutil.rmtree(TARGET / name, ignore_errors=True)
    (TARGET / ".aoc-managed").write_text("AOC managed package seed. aoc-init may refresh this directory.\n")
    print(f"synced {TARGET}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
