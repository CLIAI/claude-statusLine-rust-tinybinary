#!/usr/bin/env python3
"""Install claude-statusline-rust-tinybinary into Claude Code settings.json."""

import argparse
import json
import os
import shlex
import tempfile
import time
from pathlib import Path


DEFAULT_SETTINGS = "~/.claude/settings.json"
DEFAULT_BINARY = "claude-statusline-rust-tinybinary"


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="Add claude-statusline-rust-tinybinary to Claude Code settings.json.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""examples:
  make install && ./add-to-claude-settings.py --full --compact
  ./add-to-claude-settings.py --style default --debug-log-dir ~/.cache/claude-statusline-rust-tinybinary
  ./add-to-claude-settings.py --format '%M|%E|%T|%w|%r|%C|%c'
""",
    )
    p.add_argument(
        "--settings",
        default=DEFAULT_SETTINGS,
        help=f"Claude settings path (default: {DEFAULT_SETTINGS})",
    )
    p.add_argument(
        "--binary",
        default=DEFAULT_BINARY,
        help=f"statusline binary or path (default: {DEFAULT_BINARY})",
    )
    p.add_argument(
        "--padding",
        type=int,
        default=0,
        help="statusLine padding value (default: 0)",
    )
    p.add_argument(
        "--dry-run",
        action="store_true",
        help="print the resulting settings JSON without writing it",
    )

    style = p.add_mutually_exclusive_group()
    style.add_argument("--style", choices=["default", "full", "weekly", "debug"])
    style.add_argument("--full", action="store_true", help="use --style full")
    style.add_argument("--weekly", action="store_true", help="use --style weekly")
    style.add_argument("--debug", action="store_true", help="use --style debug")

    p.add_argument(
        "--compact",
        "-c",
        action="store_true",
        help="add compact-output modifier",
    )
    p.add_argument(
        "--reset-status",
        choices=["on", "off"],
        help="add --reset-status on|off",
    )
    p.add_argument("--format", help="add custom statusline render format")
    p.add_argument("--debug-log-dir", help="capture received Claude JSON into this directory")
    return p


def command_args(args: argparse.Namespace) -> list[str]:
    out = [args.binary]

    style = args.style
    if args.full:
        style = "full"
    elif args.weekly:
        style = "weekly"
    elif args.debug:
        style = "debug"

    if style:
        out += ["--style", style]
    if args.compact:
        out.append("--compact")
    if args.reset_status:
        out.append(f"--reset-status={args.reset_status}")
    if args.format:
        out += ["--format", args.format]
    if args.debug_log_dir:
        out += ["--debug-log-dir", str(Path(args.debug_log_dir).expanduser())]
    return out


def shell_command(parts: list[str]) -> str:
    return " ".join(shlex.quote(part) for part in parts)


def read_settings(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        with path.open("r", encoding="utf-8") as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        raise SystemExit(f"error: {path} is not valid JSON: {e}") from e
    if not isinstance(data, dict):
        raise SystemExit(f"error: {path} must contain a JSON object")
    return data


def write_settings(path: Path, data: dict, dry_run: bool) -> None:
    rendered = json.dumps(data, indent=2, sort_keys=False)
    if dry_run:
        print(rendered)
        return

    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        backup = path.with_name(f"{path.name}.bak-{time.strftime('%Y%m%d-%H%M%S')}")
        backup.write_bytes(path.read_bytes())

    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=str(path.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(rendered)
            f.write("\n")
        os.replace(tmp_name, path)
    finally:
        try:
            os.unlink(tmp_name)
        except FileNotFoundError:
            pass


def main() -> int:
    args = parser().parse_args()
    settings_path = Path(args.settings).expanduser()
    settings = read_settings(settings_path)
    command = shell_command(command_args(args))

    settings["statusLine"] = {
        "type": "command",
        "command": command,
        "padding": args.padding,
    }

    write_settings(settings_path, settings, args.dry_run)
    if not args.dry_run:
        print(f"updated {settings_path}")
        print(command)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
