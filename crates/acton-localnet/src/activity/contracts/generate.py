"""Rebuild activity assets from Acton's own templates using the latest local CLI."""

import base64
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def main():
    output = Path(__file__).resolve().parent
    root = output.parents[4]
    templates = root / "src/commands/new/templates"

    for template, contract in [("jetton-app", "JettonMinter"), ("nft-app", "NftItem")]:
        wrapper = (templates / template / "wrappers-ts" / f"{contract}.gen.ts").read_text()
        match = re.search(r"static CodeCell = c\.Cell\.fromBase64\('([^']+)'\)", wrapper)
        if match is None:
            raise ValueError(f"CodeCell missing from {contract}'s generated wrapper")
        (output / f"{contract}.boc").write_bytes(base64.b64decode(match[1]))

    # Keep the protocol change reviewable without maintaining a duplicate of the
    # whole NFT template. A changed upstream getter makes patch fail explicitly.
    with tempfile.TemporaryDirectory(prefix="acton-activity-contracts-", dir="/tmp") as directory:
        sources = Path(directory) / "contracts"
        shutil.copytree(templates / "nft/contracts", sources)
        collection = sources / "NftCollection.tolk"
        subprocess.run(
            ["patch", "--silent", str(collection), str(output / "NftCollection.patch")],
            check=True,
        )
        subprocess.run(
            [str(root / "target/debug/acton"), "compile", str(collection),
             "--boc", str(output / "NftCollection.boc")],
            cwd=root,
            check=True,
        )


if __name__ == "__main__":
    main()
