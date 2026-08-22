#!/usr/bin/env python3
"""Generate docker-compose.coolify.yml from the base compose + Supabase override.

Coolify's Docker Compose build pack reads exactly one compose file (the app's
"Docker Compose Location"), so it never picks up docker-compose.override.yml.
This flattens the two into a single file Coolify can parse.

    python3 script/gen_coolify_compose.py      # writes docker-compose.coolify.yml

Requires PyYAML and the docker compose CLI. Re-run after every upstream merge
that touches docker-compose.yml, and commit the result.

`docker compose config` is the source of truth for the merge; the fixups below
undo the things that make its output unfit to commit:

  * ``--no-interpolate`` keeps ``${VAR}`` references intact so Coolify still
    injects the deployment's environment variables.
  * bind-mount sources are re-relativised (``config`` expands them against the
    developer's checkout, e.g. /Users/you/code/...).
  * services disabled via ``profiles: ["disabled"]`` are dropped outright
    rather than left for Coolify to interpret, along with any depends_on
    references to them.
  * the synthesised project ``name`` and ``default`` network are dropped;
    Coolify supplies both.
"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "docker-compose.coolify.yml"
DISABLED_PROFILE = "disabled"

HEADER = """# GENERATED FILE - DO NOT EDIT.
#
# docker-compose.yml + docker-compose.override.yml, flattened for Coolify,
# which reads a single compose file per application. Regenerate with:
#
#     python3 script/gen_coolify_compose.py
#
# See doc/SUPABASE_AUTH.md.
"""


def merged_config() -> dict:
    compose = (
        ["docker-compose"] if shutil.which("docker-compose") else ["docker", "compose"]
    )
    out = subprocess.run(
        compose
        + [
            "-f",
            "docker-compose.yml",
            "-f",
            "docker-compose.override.yml",
            "config",
            "--no-interpolate",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return yaml.safe_load(out.stdout)


def shorten(volume):
    """Long-form mount -> "source:/target[:ro]", bind sources re-relativised."""
    if not isinstance(volume, dict):
        return volume
    source = volume.get("source")
    if volume.get("type") == "bind":
        path = pathlib.Path(source)
        try:
            source = "./" + str(path.relative_to(ROOT))
        except ValueError:
            source = str(path)
    elif volume.get("type") != "volume" or not source:
        return volume
    spec = f"{source}:{volume['target']}"
    if volume.get("read_only"):
        spec += ":ro"
    return spec


def main() -> int:
    config = merged_config()
    services = config.get("services", {})

    dropped = {
        name
        for name, svc in services.items()
        if DISABLED_PROFILE in (svc.get("profiles") or [])
    }
    for name in dropped:
        del services[name]

    for svc in services.values():
        svc.pop("profiles", None)
        svc.pop("networks", None)
        if "volumes" in svc:
            svc["volumes"] = [shorten(v) for v in svc["volumes"]]
        depends = svc.get("depends_on")
        if isinstance(depends, dict):
            for name in dropped & set(depends):
                del depends[name]
            if not depends:
                svc.pop("depends_on")

    config.pop("name", None)
    config.pop("networks", None)

    # `config` stamps each named volume with the local project's prefix
    # (appflowy-cloud_postgres_data). Coolify names volumes after its own
    # project, and hardcoding the local name would point the deployment at a
    # different volume than the one it is already using.
    for volume in (config.get("volumes") or {}).values():
        if isinstance(volume, dict):
            volume.pop("name", None)

    OUT.write_text(HEADER + yaml.safe_dump(config, sort_keys=True, width=1000))
    print(f"wrote {OUT.relative_to(ROOT)} (dropped: {', '.join(sorted(dropped)) or 'none'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
