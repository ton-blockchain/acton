import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

Manifest = dict[str, Any]
ArchiveNames = list[str]
ChecksumsByArchive = dict[str, str]
ArchiveChecksumFileMapping = dict[str, str]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Generate a cargo-dist local manifest with archive checksums.",
    )
    parser.add_argument(
        "--manifest",
        required=True,
        metavar="PATH",
        help="Path to the cargo-dist local manifest JSON to update.",
    )
    parser.add_argument(
        "--artifacts-dir",
        required=True,
        metavar="PATH",
        help="Directory containing acton-*.tar.gz.sha256 files.",
    )
    parser.add_argument(
        "--output",
        required=True,
        metavar="PATH",
        help="Path to write the cargo-dist manifest JSON.",
    )
    return parser


def load_manifest(manifest_path: Path) -> Manifest:
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"failed to parse cargo-dist manifest {manifest_path}: {exc}") from exc

    if not isinstance(manifest, dict):
        raise ValueError(f"expected top-level JSON object in {manifest_path}")

    return manifest


def read_sha256(checksum_path: Path) -> str:
    try:
        first_line = checksum_path.read_text(encoding="utf-8").splitlines()[0]
    except IndexError as exc:
        raise ValueError(f"{checksum_path} is empty") from exc

    parts = first_line.split()
    if len(parts) == 0:
        raise ValueError(f"{checksum_path} does not contain a SHA-256 checksum")

    checksum = parts[0]
    if len(checksum) != 64:
        raise ValueError(f"{checksum_path} does not contain a SHA-256 checksum")

    return checksum.lower()


def calculate_sha256(file_path: Path) -> str:
    checksum = hashlib.sha256()
    checksum.update(file_path.read_bytes())

    return checksum.hexdigest()


def collect_artifact_archives(artifacts_dir: Path) -> ArchiveNames:
    archive_paths = list(artifacts_dir.glob("acton-*.tar.gz"))
    if len(archive_paths) == 0:
        raise ValueError(f"no Acton archives found in {artifacts_dir}")

    return [archive_path.name for archive_path in archive_paths]


def collect_artifact_checksums(artifacts_dir: Path) -> ChecksumsByArchive:
    checksum_paths = sorted(artifacts_dir.glob("acton-*.tar.gz.sha256"))
    if len(checksum_paths) == 0:
        raise ValueError(f"no Acton checksum found in {artifacts_dir}")

    checksums_by_archive: ChecksumsByArchive = {}
    for checksum_path in checksum_paths:
        archive_name = checksum_path.name.removesuffix(".sha256")
        if archive_name in checksums_by_archive:
            raise ValueError(f"duplicate checksum for {archive_name}")

        checksums_by_archive[archive_name] = read_sha256(checksum_path)

    return checksums_by_archive


def archive_checksum_artifacts(manifest: Manifest) -> ArchiveChecksumFileMapping:
    artifacts = manifest.get("artifacts")
    if artifacts is None:
        raise ValueError("expected `artifacts` in cargo-dist manifest")

    checksums_by_archive: ArchiveChecksumFileMapping = {}
    for archive_name, artifact in artifacts.items():
        if not archive_name.startswith("acton-") or not archive_name.endswith(".tar.gz"):
            continue

        checksum_name = artifact.get("checksum")
        expected_checksum_name = f"{archive_name}.sha256"
        if checksum_name != expected_checksum_name:
            raise ValueError(f"expected artifact `{archive_name}` to reference checksum `{expected_checksum_name}`")

        checksum_artifact = artifacts.get(checksum_name)
        if checksum_artifact is None:
            raise ValueError(f"expected cargo-dist manifest artifacts to contain `{checksum_name}`")

        if checksum_artifact.get("kind") != "checksum":
            raise ValueError(f"expected cargo-dist manifest artifact `{checksum_name}` to be a checksum")

        checksums_by_archive[archive_name] = checksum_name

    if len(checksums_by_archive) == 0:
        raise ValueError("cargo-dist manifest did not define any .tar.gz archives with checksums")

    return checksums_by_archive


def validate_archive_checksum_mapping(
    checksums_by_archive: ChecksumsByArchive,
    checksum_files_by_archive: ArchiveChecksumFileMapping,
    archive_names: ArchiveNames,
    artifacts_dir: Path,
) -> None:
    artifact_archive_names = set(archive_names)
    checksum_archive_names = checksums_by_archive.keys()
    if artifact_archive_names != checksum_archive_names:
        raise ValueError("checksum archive names do not match artifact archive names")

    checksum_file_archive_names = checksum_files_by_archive.keys()
    if checksum_archive_names != checksum_file_archive_names:
        raise ValueError("checksum archive names do not match cargo-dist manifest archive names")

    for archive_name in archive_names:
        actual_checksum = calculate_sha256(artifacts_dir / archive_name)
        expected_checksum = checksums_by_archive[archive_name]
        if actual_checksum != expected_checksum:
            raise ValueError(
                f"checksum mismatch for {archive_name}: expected {expected_checksum}, got {actual_checksum}"
            )


def with_archive_checksums(manifest: Manifest, checksums_by_archive: ChecksumsByArchive) -> Manifest:
    updated_manifest = copy.deepcopy(manifest)
    artifacts = updated_manifest["artifacts"]

    for archive_name, checksum in checksums_by_archive.items():
        artifact = artifacts[archive_name]
        artifact.setdefault("checksums", {})["sha256"] = checksum

    return updated_manifest


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    manifest_path = Path(args.manifest)
    artifacts_dir = Path(args.artifacts_dir)
    output_path = Path(args.output)

    try:
        manifest = load_manifest(manifest_path)

        archive_names = collect_artifact_archives(artifacts_dir)
        checksums_by_archive = collect_artifact_checksums(artifacts_dir)
        checksum_files_by_archive = archive_checksum_artifacts(manifest)
        validate_archive_checksum_mapping(
            checksums_by_archive,
            checksum_files_by_archive,
            archive_names,
            artifacts_dir,
        )

        updated_manifest = with_archive_checksums(manifest, checksums_by_archive)
    except ValueError as error:
        parser.error(str(error))

    output_path.write_text(json.dumps(updated_manifest, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {output_path}")


if __name__ == "__main__":
    main()
