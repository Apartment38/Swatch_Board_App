# Release and packaging

## Arch Linux first

`packaging/arch/PKGBUILD` is the initial distro recipe. It pins the application
source to a full Git commit and fetches Rust dependencies according to
`Cargo.lock`; the compiler and platform libraries are supplied by the build
system. This is a locked source build, not a claim of bit-for-bit reproducibility.
The Git source uses `SKIP` because makepkg verifies the fixed commit instead of
an archive checksum. A release maintainer must set `_commit` to the full commit
containing the release sources before publishing the recipe, then inspect the
resulting `.SRCINFO` and source revision. Do not use a moving branch or tag as
the source pin.

The recipe's native build requirements are Rust/Cargo, Git, Clang/libclang, pkgconf,
PipeWire, Wayland and libxkbcommon development metadata. Runtime dependencies
are based on the shipped ELF linkage and dynamic graphics loader: glibc, GCC
runtime, PipeWire and libglvnd. KDE's XDG portal is an optional runtime
integration, and `kscreen` is only used by the documented output-selection
fallback. Verify these against each supported Arch snapshot before publishing;
this repository has not yet been built in an Arch clean chroot.

Build recipe verification in a clean Arch chroot is a release gate:

```sh
makepkg --verifysource
makechrootpkg -c -r "$CHROOT"
namcap PKGBUILD swatch-board-*.pkg.tar.zst
```

Review dependency licenses before every release. `Cargo.lock` pins versions
and checksums, but does not constitute an SPDX license audit. No license audit
or clean-room reproducibility comparison has been completed for Phase 6.

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
