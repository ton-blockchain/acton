import argparse
import json
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

Labels = dict[str, str]
DifferingLabels = dict[str, list[str]]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Compare labels across local Docker images and write shared labels to a file.",
    )
    parser.add_argument(
        "--image",
        action="append",
        dest="images",
        default=[],
        metavar="IMAGE_REF",
        help="Local Docker image reference to inspect; may be provided multiple times",
    )
    parser.add_argument(
        "--output",
        required=True,
        metavar="PATH",
        help="Path to write shared labels as key=value lines",
    )
    return parser


def run_command(command: Sequence[str]) -> str:
    print("+", " ".join(command), flush=True)
    result = subprocess.run(
        list(command),
        check=False,
        text=True,
        capture_output=True,
    )

    if result.returncode != 0:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        message = f"command failed with exit code {result.returncode}: {' '.join(command)}"
        raise RuntimeError(message)

    return result.stdout


def inspect_labels(image_ref: str) -> Labels:
    output = run_command(
        ["docker", "image", "inspect", image_ref, "--format", "{{ json .Config.Labels }}"],
    )
    parsed = json.loads(output or "null")
    labels = parsed if isinstance(parsed, dict) else {}

    filtered_labels: Labels = {}
    for key, value in labels.items():
        if not isinstance(value, str):
            message = f"expected label '{key}' on '{image_ref}' to be a string"
            raise ValueError(message)
        filtered_labels[key] = value

    return filtered_labels


def format_label_state(image_ref: str, labels: Labels, key: str) -> str:
    if key not in labels:
        return f"{image_ref}=<missing>"

    return f"{image_ref}={json.dumps(labels[key])}"


def compare_labels(image_refs: list[str]) -> tuple[Labels, DifferingLabels]:
    if len(image_refs) < 2:
        raise ValueError("at least two image references are required")

    labels_by_image: list[tuple[str, Labels]] = []
    for image_ref in image_refs:
        current_labels = inspect_labels(image_ref)
        labels_by_image.append((image_ref, current_labels))

    all_keys: set[str] = set()
    for _, current_labels in labels_by_image:
        all_keys.update(current_labels)

    shared_labels: Labels = {}
    differing_labels: DifferingLabels = {}
    for key in sorted(all_keys):
        states = [format_label_state(image_ref, labels, key) for image_ref, labels in labels_by_image]
        values = [labels[key] for _, labels in labels_by_image if key in labels]

        if len(values) == len(labels_by_image) and len(set(values)) == 1:
            shared_labels[key] = values[0]
            continue

        differing_labels[key] = states

    return shared_labels, differing_labels


def print_comparison_report(
    shared_labels: Labels,
    differing_labels: DifferingLabels,
) -> None:
    lines = ["# Shared labels"]

    if len(shared_labels) == 0:
        lines.append("<none>")
    else:
        lines.extend(f"{key}={shared_labels[key]}" for key in sorted(shared_labels))

    lines.append("")
    lines.append("# Different labels")

    if len(differing_labels) == 0:
        lines.append("<none>")
    else:
        for key in sorted(differing_labels):
            lines.append(f"{key}:")
            for state in differing_labels[key]:
                lines.append(f"  {state}")

    print("".join(f"{line}\n" for line in lines), end="")


def write_shared_labels_file(shared_labels: Labels, output_path: str) -> None:
    lines = [f"{key}={shared_labels[key]}" for key in sorted(shared_labels)]
    content = "".join(f"{line}\n" for line in lines)

    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(content, encoding="utf-8")


def compare_and_write_shared_labels() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if len(args.images) < 2:
        parser.error("at least two --image values are required")

    shared_labels, differing_labels = compare_labels(args.images)
    print_comparison_report(shared_labels, differing_labels)
    write_shared_labels_file(shared_labels, args.output)


def main() -> None:
    compare_and_write_shared_labels()


if __name__ == "__main__":
    main()
