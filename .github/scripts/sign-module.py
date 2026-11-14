"""Sign an external module package for IWE Code.

Usage: python3 .github/scripts/sign-module.py PACKAGE [--out OUTPUT]

The script reads the Ed25519 private key from IWE_MODULE_SIGNING_KEY in PEM
format, computes a SHA-256 digest for every packaged file except manifest.json
and manifest.sig, stores the digests in the manifest, and signs the exact
manifest bytes into manifest.sig. The private key must match the public key
embedded in the application at assets/module-public-key.bin.

Run this script on packages intended for distribution. Development packages
can skip signing by launching the application with IWE_ALLOW_UNSIGNED_MODULES.
"""

import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

MAX_PACKAGE = 128 * 1024 * 1024
MANIFEST_NAME = "manifest.json"
SIGNATURE_NAME = "manifest.sig"
EXCLUDED = {MANIFEST_NAME, SIGNATURE_NAME}


def fail(message: str) -> int:
    print(message, file=sys.stderr)
    return 1


def load_entries(package: Path) -> list[tuple[tarfile.TarInfo, bytes | None]]:
    if package.stat().st_size > MAX_PACKAGE:
        raise ValueError("The package exceeds the 128 MiB limit.")
    entries = []
    with tarfile.open(package, "r:gz") as archive:
        for member in archive.getmembers():
            handle = archive.extractfile(member)
            content = handle.read() if handle else None
            entries.append((member, content))
    return entries


def valid_entry_path(path: str) -> bool:
    parts = path.split("/")
    return (
        not path.startswith("/")
        and "\\" not in path
        and all(part not in ("", ".", "..") for part in parts)
        and len(parts) <= 16
    )


def public_key_path() -> Path:
    return Path(__file__).resolve().parents[2] / "assets" / "module-public-key.bin"


def sign_manifest(key_file: Path, manifest_bytes: bytes) -> bytes:
    public_der = subprocess.check_output(
        ["openssl", "pkey", "-in", str(key_file), "-pubout", "-outform", "DER"]
    )
    if public_der[-32:] != public_key_path().read_bytes():
        raise ValueError("Signing key does not match the embedded verification key")
    with tempfile.TemporaryDirectory(prefix="iwe-signing-") as directory:
        source = Path(directory) / MANIFEST_NAME
        source.write_bytes(manifest_bytes)
        target = Path(directory) / SIGNATURE_NAME
        subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-rawin",
                "-inkey",
                str(key_file),
                "-in",
                str(source),
                "-out",
                str(target),
            ],
            check=True,
        )
        signature = target.read_bytes()
    if len(signature) != 64:
        raise ValueError("OpenSSL produced a signature of unexpected length")
    return signature


def write_package(output: Path, entries: list[tuple[tarfile.TarInfo, bytes | None]]) -> None:
    with tarfile.open(output, "w:gz") as archive:
        for info, content in entries:
            info.mode = 0o644
            if content is None:
                info.size = 0
                archive.addfile(info)
            else:
                info.size = len(content)
                archive.addfile(info, io.BytesIO(content))


def main() -> int:
    arguments = sys.argv[1:]
    if not arguments or ("--out" in arguments and arguments.index("--out") + 1 >= len(arguments)):
        print("Usage: sign-module.py PACKAGE [--out OUTPUT]", file=sys.stderr)
        return 2
    package = Path(arguments[0])
    if not package.is_file():
        return fail(f"Package not found: {package}")
    output = package
    if "--out" in arguments:
        output = Path(arguments[arguments.index("--out") + 1])
    if not os.environ.get("IWE_MODULE_SIGNING_KEY"):
        print(
            "Set IWE_MODULE_SIGNING_KEY to the Ed25519 private key in PEM format.",
            file=sys.stderr,
        )
        return 2

    try:
        entries = load_entries(package)
    except (tarfile.TarError, ValueError) as error:
        return fail(str(error))

    names = [member.name for member, _ in entries]
    if len(names) != len(set(names)):
        return fail("The package contains duplicate entries.")
    if MANIFEST_NAME not in names:
        return fail("The package has no manifest.json.")
    if names.count(SIGNATURE_NAME) > 1:
        return fail("The package contains duplicate signatures.")

    manifest_bytes = next(
        content for member, content in entries if member.name == MANIFEST_NAME
    )
    manifest = json.loads(manifest_bytes)
    digests = {}
    total = 0
    for member, content in entries:
        path = member.name
        if not valid_entry_path(path):
            return fail(f"Unsupported entry path: {path}")
        if content is None or path in EXCLUDED:
            continue
        total += len(content)
        if total > MAX_PACKAGE:
            return fail("The unpacked package exceeds the 128 MiB limit.")
        digests[path] = hashlib.sha256(content).hexdigest()
    manifest["digests"] = digests
    manifest_bytes = json.dumps(manifest, separators=(",", ":")).encode("utf-8")

    with tempfile.TemporaryDirectory(prefix="iwe-signing-") as directory:
        key_file = Path(directory) / "private.pem"
        key_file.touch(mode=0o600)
        key_file.write_text(os.environ["IWE_MODULE_SIGNING_KEY"], encoding="ascii")
        try:
            signature = sign_manifest(key_file, manifest_bytes)
        except (ValueError, subprocess.CalledProcessError) as error:
            return fail(str(error))

    rewritten = []
    for member, content in entries:
        if member.name in EXCLUDED:
            continue
        info = tarfile.TarInfo(member.name)
        info.type = member.type
        rewritten.append((info, content))
    manifest_info = tarfile.TarInfo(MANIFEST_NAME)
    signature_info = tarfile.TarInfo(SIGNATURE_NAME)
    output_entries = [
        (manifest_info, manifest_bytes),
        (signature_info, signature),
        *rewritten,
    ]

    temporary = output.with_suffix(output.suffix + ".tmp")
    write_package(temporary, output_entries)
    temporary.replace(output)
    identifier = manifest.get("id", "?")
    version = manifest.get("version", "?")
    print(f"Signed {identifier} {version}: {len(digests)} digests, package {output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
