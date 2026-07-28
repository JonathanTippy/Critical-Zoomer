# Critical Zoomer packaging

Assistant-owned notes for building Flatpak and Debian packages from the repo root.

## Prerequisites

- **Flatpak:** `flatpak`, `flatpak-builder`, Flathub runtime/SDK 24.08, Rust SDK extension
- **Debian:** `debhelper`, `cargo`, `rustc`, `libssl-dev`, `pkg-config`
- **Icon:** `icons/assembly_chain_crosshair.png` (512×512; generated from the SVG if missing)

## Flatpak

Install the runtime, SDK, and Rust extension once:

```bash
flatpak install flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08
flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
```

Build from the repository root (manifest sources are the whole tree):

```bash
taskset -c 3-8 flatpak-builder --force-clean --repo=repo \
  build-dir packaging/flatpak/com.criticalzoomer.CriticalZoomer.yml
flatpak build-bundle repo critical-zoomer.flatpak com.criticalzoomer.CriticalZoomer
```

Install locally:

```bash
flatpak install --user critical-zoomer.flatpak
flatpak run com.criticalzoomer.CriticalZoomer
```

`cargo --offline` is preferred inside the Flatpak sandbox when a `vendor/` tree is present. Until vendored, build with network (`--disable-rofiles-fuse` / online Flatpak source fetch) or run `cargo vendor` into `vendor/` and point `.cargo/config.toml` at it.

## Debian (.deb, amd64)

From the repository root, with `packaging/debian` as the Debian directory:

```bash
export DEBIAN_DIRECTORY=packaging/debian
taskset -c 3-8 dpkg-buildpackage -b -us -uc
```

The binary package `critical-zoomer_0.0.8_amd64.deb` appears in the parent directory. Install with:

```bash
sudo dpkg -i ../critical-zoomer_0.0.8_amd64.deb
```

Installed artifacts:

| Path | Purpose |
|------|---------|
| `/usr/bin/critical_zoomer` | Binary |
| `/usr/share/applications/com.criticalzoomer.CriticalZoomer.desktop` | Launcher |
| `/usr/share/icons/hicolor/512x512/apps/com.criticalzoomer.CriticalZoomer.png` | Icon |
| `/usr/share/metainfo/com.criticalzoomer.CriticalZoomer.metainfo.xml` | AppStream metadata |

## CI validation

The GitHub Actions `packaging` job checks that required files exist, validates the desktop entry, and optionally builds a release binary artifact.
