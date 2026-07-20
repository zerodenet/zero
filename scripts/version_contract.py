#!/usr/bin/env python3
"""Manage Zero's development and release version contract.

The workspace package version identifies a build. Compatibility changes remain
under ``Unreleased`` until ``prepare-release`` stamps the tested release
version into Cargo.toml and the public compatibility ledger together.
"""

from __future__ import annotations

import argparse
import difflib
import re
import sys
from pathlib import Path


VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z][0-9A-Za-z.-]*)?$")
WORKSPACE_SECTION_RE = re.compile(
    r"(?ms)^\[workspace\.package\]\s*\r?\n(?P<body>.*?)(?=^\[|\Z)"
)
VERSION_LINE_RE = re.compile(
    r'(?m)^(?P<prefix>version\s*=\s*")[^"]+(?P<suffix>".*)$'
)
UNRELEASED_HEADING_RE = re.compile(r"(?m)^## Unreleased(?=\r?$)")
H2_RE = re.compile(r"(?m)^## [^\r\n]+(?=\r?$)")
HTML_COMMENT_RE = re.compile(r"<!--.*?-->", re.DOTALL)

ROW_MARKER = "<!-- version-contract:unreleased-row -->"
EMPTY_ROW = f"| `Unreleased` | — | 暂无待发布的兼容性变更 {ROW_MARKER} |"
EMPTY_BODY_COMMENT = "<!-- 在这里登记已实现但尚未封板的兼容性变更。 -->"


class ContractError(RuntimeError):
    """A version contract invariant was violated."""


def _read(path: Path) -> str:
    return path.read_bytes().decode("utf-8")


def _write(path: Path, content: str) -> None:
    path.write_bytes(content.encode("utf-8"))


def _newline(content: str) -> str:
    return "\r\n" if "\r\n" in content else "\n"


def _paths(root: Path) -> tuple[Path, Path]:
    return root / "Cargo.toml", root / "docs/control-plane-api/breaking-changes.md"


def _require_version(version: str, *, development: bool | None = None) -> None:
    if not VERSION_RE.fullmatch(version):
        raise ContractError(
            f"invalid version '{version}'; expected X.Y.Z or X.Y.Z-suffix"
        )
    is_development = version.endswith("-dev")
    if development is True and not is_development:
        raise ContractError("development version must end with '-dev'")
    if development is False and is_development:
        raise ContractError("release version must not end with '-dev'")


def cargo_version(content: str) -> str:
    section = WORKSPACE_SECTION_RE.search(content)
    if not section:
        raise ContractError("Cargo.toml has no [workspace.package] section")
    match = VERSION_LINE_RE.search(section.group("body"))
    if not match:
        raise ContractError("[workspace.package] has no version field")
    line = match.group(0)
    value = re.search(r'"([^"]+)"', line)
    if not value:
        raise ContractError("workspace version is malformed")
    return value.group(1)


def replace_cargo_version(content: str, version: str) -> str:
    section = WORKSPACE_SECTION_RE.search(content)
    if not section:
        raise ContractError("Cargo.toml has no [workspace.package] section")
    body = section.group("body")
    match = VERSION_LINE_RE.search(body)
    if not match:
        raise ContractError("[workspace.package] has no version field")
    start = section.start("body") + match.start()
    end = section.start("body") + match.end()
    replacement = f'{match.group("prefix")}{version}{match.group("suffix")}'
    return content[:start] + replacement + content[end:]


def _unreleased_row(content: str) -> tuple[int, int, str]:
    if content.count(ROW_MARKER) != 1:
        raise ContractError("breaking changes must contain one unreleased matrix row marker")
    marker = content.index(ROW_MARKER)
    line_start = content.rfind("\n", 0, marker) + 1
    line_end = content.find("\n", marker)
    if line_end == -1:
        line_end = len(content)
    line = content[line_start:line_end].rstrip("\r")
    if not line.startswith("| `Unreleased` |"):
        raise ContractError("unreleased row marker must be on the Unreleased matrix row")
    return line_start, line_end, line


def _unreleased_body(content: str) -> tuple[int, int, str]:
    headings = list(UNRELEASED_HEADING_RE.finditer(content))
    if len(headings) != 1:
        raise ContractError("breaking changes must contain exactly one '## Unreleased' heading")
    heading = headings[0]
    next_heading = H2_RE.search(content, heading.end())
    body_end = next_heading.start() if next_heading else len(content)
    return heading.end(), body_end, content[heading.end():body_end]


def _has_substantive_body(body: str) -> bool:
    return bool(HTML_COMMENT_RE.sub("", body).strip())


def _has_release_heading(content: str, version: str) -> bool:
    return bool(re.search(rf"(?m)^## {re.escape(version)}\r?$", content))


def _has_release_row(content: str, version: str) -> bool:
    return bool(re.search(rf"(?m)^\| `{re.escape(version)}` \|", content))


def validate_development(cargo: str, breaking: str) -> str:
    version = cargo_version(cargo)
    _require_version(version, development=True)
    _unreleased_row(breaking)
    _unreleased_body(breaking)
    if version in breaking:
        raise ContractError(
            f"development version '{version}' must not be bound into the compatibility ledger"
        )
    return version


def validate_release(cargo: str, breaking: str, version: str) -> None:
    _require_version(version, development=False)
    actual = cargo_version(cargo)
    if actual != version:
        raise ContractError(
            f"Cargo workspace version '{actual}' does not match release '{version}'"
        )
    _, _, row = _unreleased_row(breaking)
    if row != EMPTY_ROW:
        raise ContractError("release requires an empty Unreleased matrix row")
    _, _, body = _unreleased_body(breaking)
    if _has_substantive_body(body):
        raise ContractError("release requires an empty Unreleased section")
    if not _has_release_heading(breaking, version):
        raise ContractError(f"breaking changes has no '## {version}' release section")
    if not _has_release_row(breaking, version):
        raise ContractError(f"version matrix has no '{version}' release row")


def render_release(cargo: str, breaking: str, version: str) -> tuple[str, str]:
    validate_development(cargo, breaking)
    _require_version(version, development=False)
    if _has_release_heading(breaking, version) or _has_release_row(breaking, version):
        raise ContractError(f"release '{version}' is already present in breaking changes")

    row_start, row_end, unreleased_row = _unreleased_row(breaking)
    body_start, body_end, unreleased_body = _unreleased_body(breaking)
    if not _has_substantive_body(unreleased_body):
        raise ContractError("cannot prepare a release with an empty Unreleased section")

    newline = _newline(breaking)
    released_row = unreleased_row.replace("`Unreleased`", f"`{version}`", 1)
    released_row = released_row.replace(ROW_MARKER, "")
    released_row = re.sub(r"\s+\|$", " |", released_row)

    # Replace the body first because its offsets occur after the matrix markers.
    empty_body = f"{newline}{newline}{EMPTY_BODY_COMMENT}{newline}{newline}"
    released_body = f"## {version}{unreleased_body}"
    breaking = breaking[:body_start] + empty_body + released_body + breaking[body_end:]

    # Recompute the row offset after the body replacement, then reset the
    # development row and preserve its content as the new released row.
    row_start, row_end, _ = _unreleased_row(breaking)
    breaking = breaking[:row_start] + EMPTY_ROW + breaking[row_end:]
    _, row_end, _ = _unreleased_row(breaking)
    breaking = breaking[:row_end] + f"{newline}{released_row}" + breaking[row_end:]

    cargo = replace_cargo_version(cargo, version)
    validate_release(cargo, breaking, version)
    return cargo, breaking


def render_development(cargo: str, breaking: str, version: str) -> tuple[str, str]:
    _require_version(version, development=True)
    current = cargo_version(cargo)
    if current.endswith("-dev"):
        validate_development(cargo, breaking)
    else:
        validate_release(cargo, breaking, current)
    cargo = replace_cargo_version(cargo, version)
    validate_development(cargo, breaking)
    return cargo, breaking


def _show_diff(path: Path, before: str, after: str) -> None:
    sys.stdout.writelines(
        difflib.unified_diff(
            before.splitlines(keepends=True),
            after.splitlines(keepends=True),
            fromfile=str(path),
            tofile=str(path),
        )
    )


def _apply(root: Path, renderer, version: str, dry_run: bool) -> None:
    cargo_path, breaking_path = _paths(root)
    cargo = _read(cargo_path)
    breaking = _read(breaking_path)
    next_cargo, next_breaking = renderer(cargo, breaking, version)
    if dry_run:
        _show_diff(cargo_path, cargo, next_cargo)
        _show_diff(breaking_path, breaking, next_breaking)
        return
    _write(cargo_path, next_cargo)
    _write(breaking_path, next_breaking)


def check(root: Path) -> str:
    cargo_path, breaking_path = _paths(root)
    cargo = _read(cargo_path)
    breaking = _read(breaking_path)
    version = cargo_version(cargo)
    if version.endswith("-dev"):
        validate_development(cargo, breaking)
        return f"development contract is valid ({version}, Unreleased)"
    validate_release(cargo, breaking, version)
    return f"release contract is valid ({version})"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help=argparse.SUPPRESS,
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check", help="validate the current development or release state")

    start = subparsers.add_parser(
        "start-development", help="open a development cycle with an X.Y.Z-dev build version"
    )
    start.add_argument("version")
    start.add_argument("--dry-run", action="store_true")

    prepare = subparsers.add_parser(
        "prepare-release", help="stamp Unreleased changes with the tested release version"
    )
    prepare.add_argument("version")
    prepare.add_argument("--dry-run", action="store_true")

    release = subparsers.add_parser(
        "check-release", help="validate Cargo and docs against a release or tag version"
    )
    release.add_argument("version")

    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        if args.command == "check":
            print(check(root))
        elif args.command == "start-development":
            _apply(root, render_development, args.version, args.dry_run)
            print(f"development contract prepared for {args.version}")
        elif args.command == "prepare-release":
            _apply(root, render_release, args.version, args.dry_run)
            print(f"release contract prepared for {args.version}")
        elif args.command == "check-release":
            cargo_path, breaking_path = _paths(root)
            validate_release(_read(cargo_path), _read(breaking_path), args.version)
            print(f"release contract is valid ({args.version})")
    except (ContractError, OSError, UnicodeError) as error:
        print(f"version contract error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
