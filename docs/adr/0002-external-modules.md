# External modules

Status: accepted.

## Context

ADR 0001 keeps every module bundled at build time. Adding a module requires recompiling the
application. The update decision named packaging, trust, permissions, updates, and compatibility as
open questions for any external module system. Users now need to install modules that other
developers build without an application release.

## Decision

Run external modules as separate operating-system processes. Exchange one JSON document per line
over the module's stdin and stdout. Version this protocol as `api: 1` in every package manifest.

Distribute modules as `.iwe-module` archives: a gzipped tar archive with `manifest.json`,
`manifest.sig`, `binaries/<target>/<executable>` for each supported platform, and optional `assets`.
The manifest carries the module identifier, localized name and description, semantic version,
protocol version, minimum application version, entry names, declared permissions, the panel
placement, and a SHA-256 digest per packaged file. Placement selects one workspace zone: header,
top, left, right, or bottom.

Require a valid package signature before installation. `manifest.sig` holds an Ed25519 signature
over the exact manifest bytes, and the manifest digests bind every packaged file. The application
embeds the matching public key, refuses unsigned or modified packages before unpacking, and checks
each unpacked file against its digest. Distribution packages are signed with the maintainer's
private key through the repository signing script.

Keep the local registry in `modules.json` next to `workspace.json`. Install module files under the
application data directory at `modules/<id>/<version>`. Updates must increase the module version and
replace older versions. Removal deletes the module directory and its registry record.

Show declared permissions before installation and require explicit confirmation. Permissions are
declarative. Module processes run with normal user rights; the application does not sandbox them.
The interface and the public guide state this limit openly.

Modules describe their panels as a declarative widget tree. The application renders these trees with
its own components and never runs module code in the interface process. Modules localize their own
panel text; the host passes the interface language during initialization. Manifest names and
descriptions localize per language with an English fallback.

A module process runs while its module stays enabled. It starts on application start or on
activation and stops on deactivation, removal, or shutdown. Crashed modules restart at most three
times per session before the application disables them.

System modules stay bundled, enabled by default, and removable only with the application.

## Alternatives

Dynamically loaded native libraries would keep one process but require a binary compatibility
contract and conflict with the crate's unsafe-code ban.

A brokered permission model would route every file, network, and process operation through the host.
It would not constrain a hostile process that bypasses the broker, and it would multiply the
protocol surface.

Signed packages would improve provenance but require a key distribution story before launch. The
original decision deferred them; the application now embeds the module verification key, so signed
packages are required and the deferral is closed.

## Consequences

The packaging, registry, protocol, and panel schemas become public interface. The website publishes
a module development guide. The reference module under `examples/hello-module` doubles as a
template. Integration tests exercise the protocol against a compiled test module. Unsupported
protocol versions and packages without a binary for the current platform fail installation with
clear messages. Unsigned, re-signed by another key, and modified packages fail verification before
any packaged file is unpacked. Third-party module authors cannot sign packages with the embedded
key; a key distribution story for external publishers remains open.
