# FlashROM

[![CI](https://github.com/Vo-Xuan-Duong/flashrom/actions/workflows/ci.yml/badge.svg)](https://github.com/Vo-Xuan-Duong/flashrom/actions/workflows/ci.yml)

FlashROM is a **Windows-first Android ROM flashing utility** built with **Rust, Tauri 2, React 19 and TypeScript**. It replaces repetitive ADB/Fastboot terminal work with an explicit desktop workflow for device discovery, ROM inspection, partition planning, guarded writes, recovery journaling and post-ROM app restore.

> [!WARNING]
> Flashing Android partitions or wiping userdata can permanently erase data or make a device unbootable. FlashROM is designed to fail closed when important state cannot be verified, but it cannot make an incompatible ROM, defective image or unsafe OEM flashing procedure safe.

## Project status

Current application version: **0.1.0 beta candidate**.

The software-side beta feature set is implemented and the current `main` branch is validated by CI with:

```text
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
cargo fmt --check
cargo test --locked
cargo check --locked
```

Public beta publication is still gated by real-device validation in [`docs/BETA-VALIDATION.md`](docs/BETA-VALIDATION.md). CI cannot validate OEM bootloader behavior, physical USB stability, real FastbootD transitions or first boot on actual hardware.

## Desktop workspace

The current UI is organized as one desktop workspace with five primary areas:

| Workspace | Purpose |
| --- | --- |
| **Flash workspace** | Detect/select a device, load TWRP/ROM inputs, inspect the ROM, build plans and run guarded flashing workflows. |
| **ROM preparation** | Prepare `payload.bin`, OTA ZIP and `super.img` containers into normal allowlisted image inputs. |
| **ZIP inspector** | Inspect ROM ZIP contents and safely extract supported inputs without executing archive scripts. |
| **Recovery center** | Inspect persistent execution journals, verify first boot and safely restart interrupted operations from the beginning. |
| **Platform tools** | Show the exact ADB/Fastboot binaries FlashROM resolves and verify that they can execute. |

The flash workspace also exposes App Restore Profile functionality after a device has been selected.

## Core capabilities

### Device and transport

FlashROM can:

- discover devices through ADB and Fastboot;
- require explicit serial selection when multiple devices are connected;
- distinguish Android, Recovery, ADB Sideload, classic Fastboot, FastbootD and unknown Fastboot states;
- detect the current slot and single-slot vs A/B layouts;
- scope destructive commands to the selected serial with `-s SERIAL`;
- serialize protected operations globally so flash/wipe/reboot/sideload operations cannot overlap;
- stream stdout/stderr from long-running ADB, Fastboot and preparation processes to the UI.

### TWRP and recovery

- Temporary TWRP boot uses guarded `fastboot boot` and requires classic Fastboot.
- Factory Reset requires the exact confirmation phrase `WIPE`.
- ADB Sideload only starts when the selected serial is actually reported by ADB in `sideload` state.

### ROM analysis and planning

FlashROM can inspect:

- individual `.img` files;
- extracted image directories;
- Fastboot ROM directories;
- recovery/OTA ZIP files;
- `payload.bin`;
- OTA ZIP packages containing a payload;
- `super.img`.

Planning uses an explicit partition allowlist and never invents a mapping for an unknown image filename.

Live Fastboot metadata is used to resolve:

- product/codename identity;
- active slot;
- `has-slot` metadata;
- partition size;
- logical vs physical state;
- partition type;
- classic Fastboot vs FastbootD requirement;
- bootloader unlock state;
- snapshot update state.

The resulting execution pipeline is:

```text
ROM input
   ↓
ROM Analyzer / ZIP Inspector
   ↓
Product & codename compatibility
   ↓
Partition Probe
   ↓
Final Flash Plan
   ↓
Conservative Ordering
   ↓
Execution Dry Run
   ↓
SHA-256 Execution Guard
   ↓
Guarded Full-ROM Executor
   ↓
Per-step live revalidation
   ↓
Persistent journal
   ↓
Optional wipe / reboot
   ↓
Android first-boot verification
```

### Conservative partition ordering

Automatic image-based execution uses the following policy:

```text
Boot chain
  init_boot
  vendor_kernel_boot
  vendor_boot
  dtbo
  boot
  recovery
       ↓
System payload
  system
  system_ext
  product
  vendor
  odm
  system_dlkm
  vendor_dlkm
  odm_dlkm
       ↓
AVB metadata
  vbmeta_vendor
  vbmeta_system
  vbmeta
```

Unknown partition classes fail closed instead of being appended to the sequence.

## Guarded Full-ROM Executor

Full-ROM execution is enabled only after Final Plan and Execution Guard both pass. The backend rebuilds live state instead of trusting an old frontend preview.

Immediately before every partition write it revalidates:

- selected serial still exists;
- product identity has not changed;
- bootloader still reports `unlocked=yes`;
- active slot is unchanged when relevant;
- snapshot update state remains safe;
- expected Fastboot/FastbootD mode;
- partition size;
- logical/physical metadata;
- image file size;
- image SHA-256.

Physical partitions require classic Fastboot. Logical partitions require FastbootD. Mode transitions are serialized and FlashROM waits for the same selected serial to reappear before continuing.

Protected Full-ROM confirmations are:

```text
Full ROM          FLASH ROM
Full ROM + wipe   FLASH ROM WIPE
```

Manual single-image flashing requires:

```text
FLASH
```

### First-boot verification

When automatic reboot is requested, successful partition writes alone are not considered full success. FlashROM waits for the original serial to return through ADB and checks:

```text
sys.boot_completed = 1
ro.product.device
ro.build.version.release
ro.build.fingerprint
```

The default post-flash boot verification timeout is five minutes.

## Persistent journal and recovery

Every Full-ROM operation writes a persistent journal containing operation identity, device serial, ROM path, requested strategy and per-step state.

If execution is interrupted, Recovery Center **does not blind-resume at the next partition**. A retry starts from the beginning only after the backend rebuilds compatibility, partition metadata, ordering and SHA-256 Guard from the current ROM/device state.

This is intentional: device mode, slot, image files or partition metadata may have changed while the application was closed.

## Specialized ROM preparation

Container preparation is deliberately separated from partition writing. Preparation commands never flash a device.

### `payload.bin` and full OTA ZIP

Beta payload support uses a locally provisioned `payload-dumper-go` executable.

FlashROM:

1. reads the payload partition list first;
2. intersects it with FlashROM's partition allowlist;
3. extracts only supported partitions;
4. leaves payload verification enabled;
5. preserves trusted product/codename metadata where available;
6. sends the prepared directory back through the normal compatibility, Final Plan and SHA-256 Guard pipeline.

Incremental/delta OTA packages that require previous/base images are deliberately not guessed or merged automatically in the beta workflow.

### `super.img`

Raw `super.img` remains `manual_only` in Final Plan. The supported beta path is to unpack it first:

```text
super.img
   ↓
if sparse: simg2img
   ↓
lpunpack
   ↓
allowlisted logical partition images
   ↓
slot-aware Final Plan
   ↓
FastbootD + SHA-256 Guard
```

Unsupported unpacked images are quarantined in `_ignored_partitions` and do not enter automatic execution.

Explicit names such as `system_a.img` and `vendor_b.img` are only accepted when live slot metadata confirms that the corresponding partition is slot-aware. A `both` strategy is blocked when explicit A/B coverage is incomplete.

## ROM ZIP Inspector

ZIP inspection reads the central directory without executing anything inside the archive.

Safe extraction is limited to supported ROM inputs such as:

- `payload.bin`;
- `android-info.txt`;
- OTA `metadata`;
- `META-INF/com/android/metadata`;
- `.img` files.

ZIP extraction rejects path traversal and symbolic-link entries, applies entry/file/expanded-size limits and never runs `flash_all`, updater scripts or arbitrary archive programs.

## App Restore Profile

Before a clean flash, FlashROM can scan third-party Android packages and build a restore profile.

Supported restore helpers include:

- installer/source classification;
- Google Play apps delegated to Android/Google restore and verified afterwards;
- Obtainium/F-Droid/Aurora/external-store detection;
- local and sideloaded `base.apk` + split APK backup;
- SHA-256 recording for APK backups;
- `adb install` / `adb install-multiple` restore;
- package verification after restore;
- persistent `flashrom-restore-profile.json` configuration.

FlashROM deliberately does **not** directly copy app-private `/data/data` directories. UID mapping, SELinux contexts, Android Keystore and encryption make raw private-data restoration unsafe across ROM installations.

### Obtainium / F-Droid config vault

An explicitly exported Obtainium or F-Droid configuration can be copied into the restore workspace, pinned by SHA-256 and later staged to the selected ADB serial under:

```text
/sdcard/Download/FlashROM/obtainium
/sdcard/Download/FlashROM/fdroid
```

Staging requires the exact phrase:

```text
STAGE CONFIG
```

The final manager-specific import remains an explicit action inside Obtainium/F-Droid. FlashROM does not fabricate a private-data import API.

## Safety model

The beta safety boundary is intentionally conservative:

```text
No implicit device
No implicit partition
No unknown Fastboot mode
No product mismatch
No locked-bootloader writes
No active-snapshot writes
No oversized partition image
No overlapping protected operations
No unknown-image auto mapping
No raw automatic super.img write
No ZIP script execution
No blind journal resume
No unverified source-manager config staging
No automatic bootloader-firmware flashing outside the allowlist
```

Process execution is centralized in Rust and uses `std::process::Command` with argument arrays rather than constructing shell commands.

## Tool setup

FlashROM does not download ADB/Fastboot or ROM-conversion helper executables at runtime. Operators must provision trusted tools locally.

### Android Platform Tools

Resolution order:

1. `FLASHROM_PLATFORM_TOOLS`
2. `tools/platform-tools/`
3. system `PATH`

Expected Windows files in a local tools directory include:

```text
tools/platform-tools/
├── adb.exe
├── AdbWinApi.dll
├── AdbWinUsbApi.dll
└── fastboot.exe
```

The Platform Tools workspace shows the exact resolved path and executes `adb version` / `fastboot --version` to verify readiness.

### Specialized helpers

| Workflow | Environment override | Local path | PATH fallback |
| --- | --- | --- | --- |
| Payload / OTA | `FLASHROM_PAYLOAD_DUMPER` | `tools/payload-dumper-go/payload-dumper-go.exe` | `payload-dumper-go.exe` |
| Dynamic partitions | `FLASHROM_LPUNPACK` | `tools/dynamic-partitions/lpunpack.exe` | `lpunpack.exe` |
| Sparse super conversion | `FLASHROM_SIMG2IMG` | `tools/dynamic-partitions/simg2img.exe` | `simg2img.exe` |

See [`tools/README.md`](tools/README.md) for provisioning and supply-chain guidance.

## Run locally on Windows

### Prerequisites

Install:

- Windows 10 or Windows 11;
- Node.js **>= 22.12**;
- pnpm **10.15.0**;
- Rust stable MSVC toolchain;
- Microsoft Visual Studio / Build Tools with **Desktop development with C++**;
- Microsoft Edge WebView2 Runtime if it is not already available on Windows;
- Android SDK Platform Tools for device operations.

The canonical JavaScript package manager for this repository is **pnpm**. `package.json` pins `pnpm@10.15.0`, and CI/release workflows use pnpm. Use `pnpm-lock.yaml` as the frontend dependency lockfile.

### 1. Clone the repository

```powershell
git clone https://github.com/Vo-Xuan-Duong/flashrom.git
cd flashrom
```

### 2. Enable the expected pnpm version

```powershell
corepack enable
corepack prepare pnpm@10.15.0 --activate
pnpm --version
```

Expected major/minor version:

```text
10.15.x
```

### 3. Install dependencies

```powershell
pnpm install --frozen-lockfile
```

### 4. Configure Platform Tools

Option A — environment variable:

```powershell
$env:FLASHROM_PLATFORM_TOOLS="D:\Android\platform-tools"
```

Option B — copy the official Google Platform Tools files into:

```text
tools/platform-tools/
```

Option C — make `adb` and `fastboot` available through system `PATH`.

Check from the terminal if needed:

```powershell
adb version
fastboot --version
```

### 5. Run the complete desktop application

```powershell
pnpm tauri dev
```

This is the normal development command. It starts the Vite frontend through Tauri and runs the Rust backend, so ADB/Fastboot/Tauri commands are available.

If you run only:

```powershell
pnpm dev
```

Vite starts the React frontend at `http://localhost:1420`, but native Tauri backend operations such as ADB/Fastboot flashing will not work as a normal browser-only page.

## Development validation

Frontend:

```powershell
pnpm check
pnpm test
pnpm build
```

Rust/Tauri:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml
```

Full local validation:

```powershell
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo check --locked --manifest-path src-tauri/Cargo.toml
```

## Build Windows installers

Tauri bundling is enabled for **NSIS** and **MSI**.

```powershell
pnpm tauri build
```

Expected bundle directories are under:

```text
src-tauri/target/release/bundle/nsis/
src-tauri/target/release/bundle/msi/
```

Public GitHub releases use the manual `.github/workflows/release.yml` workflow. It creates a draft prerelease and requires explicit acknowledgement that:

1. the applicable hardware validation checklist has been completed;
2. the signing/unsigned installer state is understood.

Signing keys or certificates are not stored in the repository.

## Source layout

```text
flashrom/
├── src/
│   ├── main.tsx                 # React entry point
│   ├── Workspace.tsx            # top-level 5-section desktop workspace
│   ├── App.tsx                  # main device + flash workspace
│   ├── components/
│   │   ├── FlashPlanPanel.tsx
│   │   ├── FinalPlanPanel.tsx
│   │   ├── RestorePanel.tsx
│   │   ├── BetaPreparationCenter.tsx
│   │   ├── RomArchivePanel.tsx
│   │   ├── RecoveryCenter.tsx
│   │   └── PlatformToolsPanel.tsx
│   └── lib/
│       ├── tauri.ts             # primary Tauri IPC contracts
│       ├── beta.ts              # beta preparation/vault IPC contracts
│       ├── tauri.test.ts
│       └── beta.test.ts
├── src-tauri/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── src/
│       ├── android.rs           # device discovery/reboot/TWRP/wipe
│       ├── process.rs           # no-shell process execution + streaming
│       ├── platform_tools.rs    # ADB/Fastboot diagnostics
│       ├── rom.rs               # ROM classification
│       ├── zip_inspection.rs    # safe ZIP inspection/extraction
│       ├── special_tools.rs     # payload/super preparation
│       ├── compatibility.rs     # ROM/device identity validation
│       ├── partition.rs         # live partition probe
│       ├── plan.rs              # preliminary plan
│       ├── final_plan.rs        # device-resolved final plan
│       ├── ordering.rs          # conservative execution ordering
│       ├── execution_preview.rs # dry-run sequence
│       ├── execution_guard.rs   # SHA-256 + state stability guard
│       ├── executor.rs          # internal ordered execution engine
│       ├── verified_executor.rs # exposed Full-ROM + first-boot verification
│       ├── operation.rs         # global protected-operation lock
│       ├── journal.rs           # persistent execution journals
│       ├── boot_verify.rs       # Android first-boot verification
│       ├── flash.rs             # guarded manual image flash
│       ├── recovery.rs          # guarded ADB sideload
│       ├── restore.rs           # APK scan/backup/restore/verify
│       ├── restore_profile.rs   # persistent restore profile
│       ├── source_manager.rs    # Obtainium/F-Droid config vault
│       ├── lib.rs               # Tauri command registration
│       └── main.rs
├── tools/
│   ├── README.md
│   └── platform-tools/README.md
├── docs/BETA-VALIDATION.md
├── .github/workflows/
│   ├── ci.yml
│   ├── release.yml
│   ├── pin-lockfiles.yml
│   └── format-rust.yml
├── package.json
├── pnpm-lock.yaml
├── CHANGELOG.md
├── SECURITY.md
├── CONTRIBUTING.md
└── LICENSE
```

## Tauri security

The desktop window uses a restricted Tauri capability (`core:default`) and a production Content Security Policy. Native process execution stays behind registered Rust Tauri commands rather than being exposed as arbitrary shell access from the frontend.

## CI

`.github/workflows/ci.yml` runs on pushes to `main` and pull requests.

Frontend job on Ubuntu:

```text
Node 24
pnpm 10.15.0
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm build
```

Rust job on Windows:

```text
Rust stable MSVC
cargo fmt --check
cargo test --locked
cargo check --locked
```

Dependency graph updates are pinned by `.github/workflows/pin-lockfiles.yml` for `pnpm-lock.yaml` and `src-tauri/Cargo.lock`.

## Beta limitations and external release gates

The following constraints are intentional for 0.1.0:

- raw `super.img` is never automatically written; unpack it through ROM Preparation first;
- incremental/delta payload OTAs that need previous images are not automatically reconstructed;
- unknown partition images and bootloader/vendor firmware outside the explicit allowlist are blocked;
- private Android `/data/data` restore is not implemented;
- Obtainium/F-Droid final config import remains explicit inside the manager app;
- external helper binaries are operator-provisioned and are not downloaded at runtime;
- real-device validation must be completed before publishing the public beta;
- Windows installer signing requires external credentials/certificates;
- the repository still uses temporary/fallback product artwork unless final release artwork is added.

Before testing destructive operations, read [`docs/BETA-VALIDATION.md`](docs/BETA-VALIDATION.md) and use disposable/test hardware with a known recovery path.

## Related documentation

- [`docs/BETA-VALIDATION.md`](docs/BETA-VALIDATION.md) — real-device beta release gate.
- [`tools/README.md`](tools/README.md) — external helper provisioning and supply-chain policy.
- [`SECURITY.md`](SECURITY.md) — project security model and disclosure guidance.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development contribution guidance.
- [`CHANGELOG.md`](CHANGELOG.md) — beta feature history.

## License

FlashROM is licensed under the [MIT License](LICENSE).
