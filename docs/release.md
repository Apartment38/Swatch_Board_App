# Release and packaging

## Arch Linux first

`packaging/arch/PKGBUILD` is the initial distro recipe. It pins the application
source payload to `efb069a24472e6336eedfe00ba729fc5585a5d8d` (`0.1.0-2`) and fetches Rust dependencies according to
`Cargo.lock`; the compiler and platform libraries are supplied by the build
system. This is a locked source build, not a claim of bit-for-bit reproducibility.
The Git source uses `SKIP` because makepkg verifies the fixed commit instead of
an archive checksum. A release maintainer must set `_commit` to the full commit
containing the release sources before publishing the recipe, then inspect the
resulting `.SRCINFO` and source revision. Do not use a moving branch or tag as
the source pin.

The recipe's native build requirements are Rust/Cargo, Git, Clang/libclang, pkgconf,
PipeWire, Wayland and libxkbcommon development metadata. Runtime dependencies
include glibc, GCC runtime, PipeWire, libglvnd, and hicolor-icon-theme. The latter
is required for the installed icon hierarchy. `options=('!lto')` is deliberate:
Arch's GCC LTO C objects for bundled SQLite cannot be consumed by Rust's LLVM
linker. This was reproduced in the isolated build and fixed before packaging.
KDE's XDG portal is an optional runtime integration, and `kscreen` is only used
by the documented output-selection fallback.

A fresh Arch Docker container built the repaired payload, ran **180 tests**
(one intentionally ignored) and strict Clippy, produced `0.1.0-2`, installed it
**inside the container**, and passed installed picker, all four plugin golden,
complete plugin/schema/license byte-inventory and metadata checks. The physical
Board/Settings keyboard/mouse, save/reopen, validation and cleanup round passed
using a host-built release from the same repaired Rust source; the newly
container-built package bytes were not themselves GUI-tested. No host Cargo
cache/target, desktop sockets or other host mounts were provided to the container.
A local Git bundle supplied the unpushed source revision via a makepkg Git URL
rewrite; this is not a test of public source availability. Push only when
explicitly authorised before remote package consumers need the source.

Current evidence: `docs/evidence/phase6-packaging/refresh-checks.json` and
`physical-keyboard.json`. `checks.json` describes the earlier `0.1.0-1` baseline,
which does not contain the Mixer feedback repair and is superseded as a package
candidate. The unsigned local artifact is under
`target/phase6-release/refreshed/swatch-board-0.1.0-2-x86_64.pkg.tar.zst`.
The build container was removed. Container verification is not official devtools
clean-chroot qualification or bit-for-bit reproducibility.

Build recipe verification in a clean Arch chroot is a release gate:

```sh
makepkg --verifysource
makechrootpkg -c -r "$CHROOT"
namcap PKGBUILD swatch-board-*.pkg.tar.zst
```

Review dependency licenses before every release. `THIRD_PARTY_NOTICES.txt` is a
conservative collection for all 431 registry packages in the current lockfile,
including build-time and non-Linux dependencies. It contains matching license,
copyright and notice files from checksum-verified crate archives, the egui
bundled font text files, and where available license files retrieved from the
exact upstream Git commit recorded in the crate. The installer includes this
collection alongside the project license. Its presence is not a completed
license audit: filename-based collection can miss other notices or obligations.
The collection explicitly marks eight packages for which no notice file was
found. The default x86_64 and aarch64 Linux production and build dependency
trees do not select any of them; `hexf-parse` is an optional dependency in the
broader metadata inventory and declares CC0-1.0. See
`docs/third-party-license-review.md` for the scope and evidence. Recollect and
review the notices whenever the lockfile, target or features change.
The Arch recipe still pins an older source commit, so its pin must be updated to
a commit containing the notice collection before a package can include it.

`Cargo.lock` pins versions
and checksums, but does not constitute an SPDX/license-notice audit. The recorded
`dependency-license-inventory.json` enumerates metadata declarations and candidate
notice-file gaps only; some upstream crate archives omit their own notice files.
The installed project MIT license is not proof that all bundled code/font
redistribution obligations are satisfied. **The existing artifact is not
approved for public release.** Each distribution candidate must have its own
native-library, toolchain and license review; AppImage and Flatpak artifacts have
not yet been built or reviewed. An independent reproducibility comparison has
also not been completed.

The installer includes the Rust standard library copyright inventory and license
texts from the active toolchain's `share/doc/rust` directory. If the build
toolchain is no longer active, set `RUST_LICENSE_DIR` to the preserved directory
from the exact toolchain that built the binaries. Installation fails if that
documentation is missing. Do not substitute a different toolchain's notices.

## Local release verification

On a host with the locked crate sources cached, run:

```sh
cargo build --workspace --locked --offline
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
packaging/verify-release.sh
```

`verify-release.sh` builds the release workspace, runs installer contract tests,
stages the actual release executables, checks the installed picker help/version,
and invokes the installed theme worker for all four installed plugin golden
fixtures. It verifies all installed plugin/schema bytes and the license. It
does not launch a GUI, open D-Bus, alter desktop settings, or grant portal
permissions. Desktop-file and AppStream validators are mandatory: missing tools
fail the gate before building. This local gate is not a clean-chroot build or
physical desktop acceptance.

For a distro build, install the six executables beside each other. The theme
worker is an internal helper and intentionally has no desktop launcher. The
installer places plugins and schemas under `share/swatch-board`, the icon in
the hicolor scalable application directory, five launchers in
`share/applications`, AppStream data in `share/metainfo`, and the MIT license in
`share/licenses/swatch-board`. Autostart remains opt-in and uses one per-user
XDG desktop entry; no system service or second autostart mechanism is installed.

## Version policy

- All workspace crates and shipped executables share the version in their
  Cargo manifests. Update them together, keep `Cargo.lock` committed, and keep
  AppStream's release version/date and Arch `pkgver`/source pin in step.
- Plugin package `version` describes its contents; `api_version` selects the
  host contract. API 1 is strict about unknown manifest fields and does not
  imply automatic project migration. Preserve a package's plugin ID and
  increment its version for content changes. Increase `api_version` only for
  an incompatible authoring/runtime contract, with a schema, migration policy,
  and compatibility tests.
- The theme-worker protocol's integer `version` is separately versioned. A
  host and helper are built and installed from one workspace revision; change
  the protocol only with coordinated reader/writer compatibility tests.
- The D-Bus API is documented in `docs/ipc.md`. Additive methods/signals may be
  introduced compatibly; do not change existing signatures or meanings without
  a new interface version and migration period.

## Compatibility targets and release gates

The supported physical target is KDE Plasma 6 on Wayland with a working
PipeWire ScreenCast portal and GlobalShortcuts portal. Fedora and openSUSE are
compatibility targets to evaluate after Arch; their package names, portal
versions, dependencies, and clean-build results have not been validated.
Record distro release, kernel, Rust version, Plasma/KWin version, portal
backend/version, GPU driver, displays/scales, and test result for each matrix
run. Do not infer support for another compositor from Wayland protocol support.

Never test compositor recovery by restarting the live `kwin_wayland` service.
That action previously ended the user's graphical session and prevented
re-login until reboot. The safe substitute is the already documented private
KWin session for automated restart/recovery checks, followed by a normal
physical logout/login performed manually for desktop continuity. This does
not test recovery from an in-place restart on the user's desktop; record that
gate as unavailable rather than retrying the hazardous operation.
