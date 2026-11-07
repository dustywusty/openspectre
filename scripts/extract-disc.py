#!/usr/bin/env python3
"""Extract the supplied CD through hfsutils; retain both Mac file forks."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FILES = {
    "spectre": "Spectre VR CD",
    "central": "Central Systems Matrix",
    "eastern": "Eastern Edge Matrix",
    "south": "South Central Matrix",
    "textures": "Textures",
    "vrchitect": "VRchitect",
}


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("iso", type=Path)
    args = parser.parse_args()
    source = args.iso.resolve()
    for tool in ("hmount", "hcopy"):
        if not shutil.which(tool):
            raise SystemExit("Install hfsutils first: brew install hfsutils")
    local = ROOT / ".local"
    media = local / "media"
    out = local / "extracted"
    media.mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    copy = media / "SpectreVRCD_1_1.iso"
    if source == copy.resolve():
        raise SystemExit("Supply the original ISO, outside .local/media.")
    original_hash = digest(source)
    shutil.copyfile(source, copy)
    # hfsutils keeps volume selection in .hcwd. Restore the prior selection.
    hcwd = Path.home() / ".hcwd"
    previous = hcwd.read_bytes() if hcwd.exists() else None
    try:
        subprocess.run(["hmount", str(copy)], check=True)
        for stem, name in FILES.items():
            subprocess.run(["hcopy", "-m", f":Spectre VR CD:{name}", str(out / f"{stem}.bin")], check=True)
        subprocess.run(["hcopy", "-t", ":Read Me!", str(out / "readme.txt")], check=True)
    finally:
        if previous is None:
            hcwd.unlink(missing_ok=True)
        else:
            hcwd.write_bytes(previous)
    if digest(source) != original_hash:
        raise SystemExit("Original ISO hash changed during extraction.")
    manifest = {"source": str(source), "sha256": original_hash,
                "files": {stem: {"hfs_path": f":Spectre VR CD:{name}",
                                 "sha256": digest(out / f"{stem}.bin"),
                                 "bytes": (out / f"{stem}.bin").stat().st_size}
                          for stem, name in FILES.items()}}
    (out / "provenance.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Extracted {len(FILES)} MacBinary files to {out}")
    print(f"Original ISO unchanged: {original_hash}")


if __name__ == "__main__":
    main()
