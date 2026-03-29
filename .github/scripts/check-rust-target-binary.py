import argparse
import json
import re
import subprocess
from typing import NamedTuple

_TARGET_PATTERN: re.Pattern[str] = re.compile(r"^(?P<arch>[^-]+)-(?P<vendor>[^-]+)-(?P<sys>[^-]+)(?:-(?P<abi>[^-]+))?$")

_TARGET_MAP: dict[str, dict[str, str]] = {
    "x86_64-unknown-linux-gnu": {
        "GLIBC": "2.34",
        "GLIBCXX": "3.4.29",
        "OPENSSL": "3.0.0",
    },
    "aarch64-unknown-linux-gnu": {
        "GLIBC": "2.34",
        "GLIBCXX": "3.4.29",
        "OPENSSL": "3.0.0",
    },
    "x86_64-apple-darwin": {
        "LC_VERSION_MIN_MACOSX.version": "10.12",
    },
    "aarch64-apple-darwin": {
        "LC_BUILD_VERSION.minos": "11.0",
    },
}

_OTOOL_PATH = "/usr/bin/otool"
_STRINGS_PATH = "/usr/bin/strings"

_VERSION_PATTERNS: dict[str, re.Pattern[str]] = {
    "GLIBC": re.compile(r"\bGLIBC_(?P<version>\d+\.\d+(?:\.\d+)?)\b"),
    "GLIBCXX": re.compile(r"\bGLIBCXX_(?P<version>\d+\.\d+(?:\.\d+)?)\b"),
    "OPENSSL": re.compile(r"\bOPENSSL_(?P<version>\d+\.\d+(?:\.\d+)?)\b"),
}


class RustTarget(NamedTuple):
    arch: str
    vendor: str
    sys: str
    abi: str | None

    def format(self) -> str:
        base_target = f"{self.arch}-{self.vendor}-{self.sys}"
        if self.abi is None:
            return base_target

        return f"{base_target}-{self.abi}"


class OtoolParser:
    def __init__(self, output: str) -> None:
        self.output = output

    @staticmethod
    def _run(binary_path: str, arch: str) -> str:
        command = [_OTOOL_PATH, "-arch", arch, "-l", binary_path]

        try:
            result = subprocess.run(
                command,
                capture_output=True,
                text=True,
                check=False,
            )
        except FileNotFoundError as error:
            message = "otool is not available"
            raise ValueError(message) from error

        if result.returncode != 0:
            message = result.stderr.strip()
            if len(message) == 0:
                message = f"otool failed for '{binary_path}'"
            raise ValueError(message)

        return result.stdout

    @classmethod
    def from_binary_path(cls, binary_path: str, arch: str) -> "OtoolParser":
        return cls(cls._run(binary_path, arch))

    def parse_field(self, block_name: str, field_name: str) -> str:
        in_block = False
        field_prefix = f"{field_name} "

        for line in self.output.splitlines():
            stripped = line.strip()

            if stripped.startswith("cmd "):
                in_block = stripped == f"cmd {block_name}"
                continue

            if in_block and stripped.startswith(field_prefix):
                return stripped.removeprefix(field_prefix).strip()

        message = f"unable to find '{field_name}' in '{block_name}' block"
        raise ValueError(message)


class StringsParser:
    VERSION_GROUP_NAME = "version"

    def __init__(self, output: str) -> None:
        self.output = output

    @staticmethod
    def _run(binary_path: str) -> str:
        try:
            result = subprocess.run(
                [_STRINGS_PATH, "-a", "--", binary_path],
                capture_output=True,
                text=True,
                check=False,
            )
        except FileNotFoundError as error:
            message = "strings is not available"
            raise ValueError(message) from error

        if result.returncode != 0:
            message = result.stderr.strip()
            if len(message) == 0:
                message = f"strings failed for '{binary_path}'"
            raise ValueError(message)

        return result.stdout

    @classmethod
    def from_binary_path(cls, binary_path: str) -> "StringsParser":
        return cls(cls._run(binary_path))

    def parse_versions(self, version_pattern: re.Pattern[str]) -> list[str]:
        pattern = re.compile(version_pattern)
        versions = {match.group(self.VERSION_GROUP_NAME) for match in pattern.finditer(self.output)}

        if len(versions) == 0:
            message = f"unable to find version matching '{pattern.pattern}' in strings output"
            raise ValueError(message)

        return sorted(versions, key=self._parse_version_key)

    @staticmethod
    def _parse_version_key(value: str) -> tuple[int, ...]:
        return tuple(int(part) for part in value.split("."))

    def parse_symbol_versions(self, symbol_name: str) -> list[str]:
        try:
            pattern = _VERSION_PATTERNS[symbol_name]
        except KeyError as error:
            message = f"unsupported symbol name: '{symbol_name}'"
            raise ValueError(message) from error

        return self.parse_versions(pattern)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Parse a Rust target string for GitHub Actions")
    parser.add_argument(
        "--binary",
        required=True,
        metavar="PATH",
        help="Path to the binary artifact",
    )
    parser.add_argument(
        "--target",
        required=True,
        metavar="RUST_TARGET",
        help="Rust target string, for example x86_64-unknown-linux-gnu",
    )
    return parser


def parse_target(value: str) -> RustTarget:
    match = _TARGET_PATTERN.fullmatch(value)
    if match is None:
        message = f"unable to parse Rust target: '{value}'"
        raise ValueError(message)

    groups = match.groupdict()
    return RustTarget(
        arch=groups["arch"],
        vendor=groups["vendor"],
        sys=groups["sys"],
        abi=groups["abi"],
    )


def _run_linux_checks(target: RustTarget, binary_path: str) -> list[str]:
    target_key = target.format()
    expected_versions = _TARGET_MAP[target_key]
    parser = StringsParser.from_binary_path(binary_path)

    errors: list[str] = []
    for symbol_name, expected_version in expected_versions.items():
        try:
            versions = parser.parse_symbol_versions(symbol_name)
            actual_version = versions[-1]
        except ValueError as error:
            errors.append(f"linux check failed for '{target_key}': {error}")
            continue

        if actual_version != expected_version:
            errors.append(
                (
                    f"linux check failed for '{target_key}': '{symbol_name}', "
                    f"expected '{expected_version}', actual '{actual_version}', "
                    f"possible versions {versions}"
                ),
            )

    return errors


def _create_apple_parser(target: RustTarget, binary_path: str) -> OtoolParser:
    if target.arch == "x86_64":
        return OtoolParser.from_binary_path(binary_path, "x86_64")

    if target.arch == "aarch64":
        return OtoolParser.from_binary_path(binary_path, "arm64")

    return OtoolParser.from_binary_path(binary_path, target.arch)


def _run_apple_checks(target: RustTarget, binary_path: str) -> list[str]:
    target_key = target.format()
    expected_values = _TARGET_MAP[target_key]
    parser = _create_apple_parser(target, binary_path)

    errors: list[str] = []
    for apple_key, expected_value in expected_values.items():
        parts = apple_key.split(".", maxsplit=1)
        if len(parts) != 2:
            message = f"invalid apple key: '{apple_key}'"
            raise ValueError(message)

        try:
            actual_value = parser.parse_field(parts[0], parts[1])
        except ValueError as error:
            errors.append(f"apple check failed for '{target_key}': {error}")
            continue

        if actual_value != expected_value:
            errors.append(
                (
                    f"apple check failed for '{target_key}': '{apple_key}', "
                    f"expected '{expected_value}', actual '{actual_value}'"
                ),
            )

    return errors


def run_target_checks(target: RustTarget, binary_path: str) -> list[str]:
    if target.sys == "linux":
        return _run_linux_checks(target, binary_path)

    if target.vendor == "apple":
        return _run_apple_checks(target, binary_path)

    return [f"unsupported target: '{target.format()}'"]


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    try:
        target = parse_target(args.target)
    except ValueError as error:
        parser.error(str(error))

    errors = run_target_checks(target, args.binary)
    if len(errors) != 0:
        parser.error("\n".join(errors))

    print(json.dumps(target._asdict()))


if __name__ == "__main__":
    main()
