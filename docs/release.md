# Release and packaging

## Arch Linux first

`packaging/arch/PKGBUILD` is the initial distro recipe. The `0.1.0-4` recipe pins
the local custom-effect source checkpoint `bd3b66726bf78047cf0491ae1d62f16386f36c93`.
The recipe fetches Rust dependencies according to `Cargo.lock`;
the build system supplies the compiler and platform libraries. This is a locked
source build, not a claim of bit-for-bit reproducibility.
The Git source uses `SKIP` because makepkg verifies the fixed commit instead of
an archive checksum. A release maintainer must set `_commit` to the full commit
containing the release sources before publishing the recipe, then inspect the
resulting `.SRCINFO` and source revision. Do not use a moving branch or tag as
the source pin.

The recipe's native build requirements are Rust/Cargo, a C++ compiler, CMake,
extra-cmake-modules, Qt 6 and matching KWin development headers, Git, pkgconf,
Vulkan headers, Wayland and libxkbcommon development metadata. Cargo's shared
`swatch-board-qt` crate additionally compiles C++17 Qt Widgets sources using
`pkg-config Qt6Widgets` and `ar`. Runtime dependencies include glibc, GCC runtime,
KWin, Qt 6 Base/Wayland, Plasma Qt integration, Breeze, and hicolor-icon-theme.
No Qt shared libraries, themes, or fonts are bundled by the installer.
`options=('!lto')` is deliberate: Arch's GCC LTO C objects for bundled SQLite
cannot be consumed by Rust's LLVM linker. KDE's XDG portal provides optional
GlobalShortcuts integration; the picker uses KWin directly, not ScreenCast.

Historically, a fresh Arch Docker container built the repaired payload, ran **180 tests**
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

Historical evidence: `docs/evidence/phase6-packaging/refresh-checks.json` and
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

Review dependency licenses before every release. `THIRD_PARTY_NOTICES.txt`
contains all 161 registry packages in the Qt cutover lockfile, including
build-time and non-Linux dependencies. Obsolete renderer/font notice sections
were removed; retained archive checksums and notices were preserved unchanged.
It contains license/copyright files from checksum-verified crate archives and,
where available, the exact upstream Git commit recorded in the crate.
The installer includes this collection alongside the project license.
This is not a completed license audit: filename-based collection can miss
other notices or obligations. The retained `r-efi` record is marked unavailable;
reassess target/feature selection before redistribution. Qt/KDE native library
licenses are separate from Cargo's inventory. See
`docs/third-party-license-review.md`; recollect/review whenever dependencies change.
The `0.1.0-4` source checkpoint includes the custom effect and its GPL license.
The source has not been pushed and this package revision has not been built or
release-qualified.

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
cmake -S kwin -B target/kwin -DCMAKE_BUILD_TYPE=Release
cmake --build target/kwin
packaging/verify-release.sh
```

`verify-release.sh` builds the release workspace and KWin effect, runs installer
contract tests, stages the actual payload, checks the installed picker
help/version, and invokes the installed theme worker for all four plugin golden
fixtures. It verifies effect, license, and installed plugin/schema bytes. It
does not launch a GUI, open D-Bus, alter desktop settings, or grant portal
permissions. Desktop-file and AppStream validators are mandatory: missing tools
fail the gate before building. This local gate is not a clean-chroot build or
physical desktop acceptance.

Install the six executables beside each other; the theme worker has no desktop
launcher. Install the required `target/kwin/lib/swatchboardpicker.so` under the
Qt plugin root at `kwin/effects/plugins/swatchboardpicker.so`. The installer
discovers the root using Qt tooling or pkg-config; `QT_PLUGIN_DIR` overrides it,
`EFFECT_LIB` overrides the built effect, and `DESTDIR` stages absolute paths.
`PREFIX` controls binaries and data. Missing effects fail installation. The
payload also includes plugins, schemas, five launchers, icon, AppStream data,
MIT and `kwin/LICENSE` GPL-2.0-or-later licenses, and Rust notices.
Test the built plugin in an isolated KWin first. System installation and loading
require user confirmation; never restart live KWin. KWin's effect ABI is tied to
the installed version: rebuild against matching development files after upgrades.
Autostart remains opt-in via one per-user XDG desktop entry.

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

The supported physical target is KDE Plasma 6 on Wayland with the custom KWin effect.
The tray's global shortcuts require a working GlobalShortcuts portal. Native
acceptance of the replacement picker is pending. Fedora and openSUSE are
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
