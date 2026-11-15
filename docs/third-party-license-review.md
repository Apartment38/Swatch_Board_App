# Third-party licensing review

The detailed review below applies to the pre-KWin-picker lockfile with SHA-256
`210d1cc0deea8a0c78d7c74b05d69ba358e34b7ea81e034c9c5e5d0268b0e035`.
The KWin cutover removed 25 registry packages and preserved the remaining 406
notice sections unchanged; it added no registry packages. The new lockfile SHA-256
is `6a0fb3f9871021866d56f3cab308e4acdd9dcd84c0384504e508c5e86e97e175`.
The dependency trees and binary linkage observations below are historical, not
fresh qualification of the replacement picker or a release approval.

## Rust packages and embedded assets

At that checkpoint the notice collection conservatively covered all 431 registry
packages, including optional, test, build and non-Linux packages. Crate
archives were checked against their lockfile SHA-256 checksums. Where archives
omitted their own license files, the collection includes files obtained from
the source commit identified in the crate where available. A filename search
is not proof that all nested third-party material or attribution has been found.

The following commands examine the resolved default feature trees for each
supported architecture (they require a populated Cargo cache for `--offline`):

```sh
cargo tree --locked --offline --workspace --target x86_64-unknown-linux-gnu --edges normal,build
cargo tree --locked --offline --workspace --target aarch64-unknown-linux-gnu --edges normal,build
```

Neither tree includes `block`, `dispatch`, `malloc_buf`, `hexf-parse`, `r-efi`,
`winapi-i686-pc-windows-gnu`, or `winapi-x86_64-pc-windows-gnu`, the eight
package/version records marked as missing a notice file in the collection.
`hexf-parse` declares CC0-1.0, and its recorded upstream source commit has no
license file. It appears in Cargo's broader metadata resolution because optional
dependencies are represented there. If features or targets change, reassess
these missing files before distribution. For both Linux targets, the default
normal dependency tree contains 240 registry package versions and 12 workspace
packages; every registry package in that tree has a top-level license file in
its verified crate archive or an upstream license file captured at its recorded
source commit. This is a file-presence check, not a complete assessment of every
file in every package.

`epaint_default_fonts` embeds Hack, Noto Emoji, Ubuntu Light, and emoji-icon
fonts. The notice collection contains their supplied license texts and the
copyright strings in their font metadata. Preserve the font license terms and
names if changing or redistributing these files. `rusqlite` enables bundled
SQLite; `libsqlite3-sys` 0.30.1 includes SQLite 3.46.0, whose source disclaims
copyright. The collection includes the source notice. SQLCipher is a distinct
feature and is not selected by the default build.

The Rust standard library is not represented by `Cargo.lock`. The installer
requires and installs `COPYRIGHT-library.html` and the license directory from the
build toolchain. The release maintainer must ensure that these files come from
the exact toolchain used to compile the executables.

## Linux build inspection

An x86_64 Debian 13 container built the locked workspace in release mode with
Rust 1.100.0 (`0ef1bc386059596dada8fb65b50425aab2ea637a`). The installer
staged all six executables and their notice files using that toolchain's Rust
documentation; the staged picker reported version 0.1.0. `readelf -d` reported
direct dependencies on `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, and
`ld-linux-x86-64.so.2` for each executable, plus `libpipewire-0.3.so.0` for
the picker. The build environment provided glibc 2.41-12+deb13u4, libgcc
14.2.0-19 and PipeWire 1.4.2-1. These are dynamically linked and were not
copied into the staged installation. This is evidence for that build only;
`readelf` does not reveal libraries loaded later via `dlopen`, and these files
are not the Arch, Flatpak, or AppImage release artifacts.

## Remaining artifact-specific work

An Arch package that declares system dependencies does not itself bundle those
shared libraries; its final file list and linked library dependencies still need
review. AppImage or Flatpak builds may include additional libraries, runtimes,
fonts, graphics drivers, or other data. For each final artifact, inventory its
actual files and linkage, identify exact versions and licenses, include required
copyright and notices, and satisfy any source, relinking, offer, or other
obligations of the licenses actually redistributed. Recheck toolchain notices,
feature selection, and lockfile for each architecture. Do not treat the Rust
crate collection as a substitute for this artifact-specific review.
