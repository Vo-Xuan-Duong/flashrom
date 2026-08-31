# Changelog

All notable changes to FlashROM will be documented here.

## [Unreleased]

### Added

- multi-device discovery and explicit serial selection;
- global protected-operation serialization;
- guarded Full-ROM Executor with conservative ordering;
- per-step serial/product/slot/mode/partition/SHA-256 revalidation;
- persistent Full-ROM execution journals;
- Recovery Center for journal inspection and safe retry-from-beginning;
- Android post-flash boot verification using `sys.boot_completed` and device identity;
- ROM ZIP central-directory inspection and safe extraction of metadata, images and `payload.bin`;
- Android Platform Tools readiness/version diagnostics;
- App Restore Profile with local/split APK backup, SHA-256 and restore verification;
- production CSP and Windows NSIS/MSI bundle configuration;
- manual Windows GitHub Release workflow.

### Safety

- ZIP extraction rejects path traversal and symbolic-link entries;
- automatic payload partition extraction remains blocked until update_engine payload parsing is fully validated;
- `super.img` remains manual-only;
- interrupted operations never blind-resume from the next partition; retry rebuilds the complete Guard from current device/ROM state.

## [0.1.0] - Unreleased beta

Initial Windows-first beta milestone.
