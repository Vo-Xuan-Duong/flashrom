# Changelog

All notable changes to FlashROM will be documented here.

## [Unreleased]

### Changed

- the 0.1.0 software feature set is now beta-ready; publication remains gated by real-device validation and installer signing policy.

## [0.1.0] - Beta candidate

### Added

- multi-device ADB/Fastboot discovery with explicit serial selection;
- Android, Recovery, ADB Sideload, classic Fastboot and FastbootD detection;
- single-slot/A/B boot-layout detection and active-slot handling;
- TWRP temporary boot, guarded ADB Sideload and exact-confirmation Factory Reset;
- global protected-operation serialization;
- ROM analyzer, ZIP central-directory inspector and safe ROM-input extraction;
- live partition probe for slot support, partition size, type and logical/physical state;
- ROM product/codename compatibility checks;
- device-resolved Final Flash Plan and conservative boot-chain/system/AVB ordering;
- explicit slot-qualified prepared images such as `system_a.img` / `system_b.img` with live slot validation;
- immutable SHA-256 Execution Guard;
- guarded Full-ROM Executor with per-step serial/product/slot/mode/partition/hash revalidation;
- serialized Fastboot ↔ FastbootD transitions;
- realtime stdout/stderr streaming for long-running ADB/Fastboot/helper operations;
- persistent Full-ROM execution journals;
- Recovery Center for journal inspection and safe retry-from-beginning;
- Android post-flash boot verification using `sys.boot_completed`, product identity, Android release and build fingerprint;
- Android Platform Tools readiness/version diagnostics;
- specialized helper diagnostics for `payload-dumper-go`, `lpunpack` and `simg2img`;
- guarded full-OTA / `payload.bin` preparation that lists payload partitions first, extracts only FlashROM's allowlist and leaves extractor verification enabled;
- guarded `super.img` preparation through optional `simg2img` followed by `lpunpack`;
- quarantine of unpacked partition images outside FlashROM's beta allowlist;
- preservation of trusted ROM identity metadata into prepared workspaces;
- App Restore Profile with local/split APK backup, SHA-256 and restore verification;
- Obtainium/F-Droid configuration vault with SHA-256 pinning and explicit ADB staging to Downloads;
- Beta Preparation Center UI for payload/super preparation and source-manager restore staging;
- frontend Vitest IPC-contract coverage for destructive and beta preparation commands;
- pinned `pnpm-lock.yaml` and `src-tauri/Cargo.lock` dependency graphs;
- production CSP and Windows NSIS/MSI bundle configuration;
- manual Windows GitHub Release workflow gated on hardware-validation and unsigned-installer acknowledgement;
- documented external helper provisioning and a comprehensive real-device beta validation checklist.

### Safety

- no implicit device or partition selection;
- destructive commands remain scoped to the selected serial;
- no writes when bootloader unlock, product identity, snapshot state, partition metadata or device mode cannot be confirmed;
- no automatic mapping for unknown image filenames;
- no automatic raw `super.img` write;
- both-slot execution does not invent slot targets and blocks incomplete explicit `_a/_b` image sets;
- ZIP extraction rejects path traversal and symbolic-link entries and never executes embedded scripts;
- payload extraction does not disable payload verification and does not guess previous/base images for incremental OTAs;
- specialized helper binaries are never downloaded at runtime;
- unsupported prepared partitions are quarantined instead of entering the automatic plan;
- interrupted Full-ROM operations never blind-resume from the next partition;
- source-manager configuration is hash-verified before staging and FlashROM does not write private `/data/data` application files;
- Full-ROM success with reboot requires verified Android first boot, not merely successful partition writes.

### Release gates outside CI

- execute the applicable rows in `docs/BETA-VALIDATION.md` on disposable/test hardware;
- document whether public Windows installers are signed;
- optionally replace temporary/fallback artwork before broad distribution.
