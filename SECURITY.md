# Security Policy

FlashROM can execute ADB/Fastboot commands that may erase data or make an Android device unbootable. Treat security and device-targeting bugs as high impact.

## Supported versions

The latest code on `main` and the latest published beta release are supported during the pre-1.0 period.

## Reporting a vulnerability

Please do not publish a working destructive exploit before maintainers have had a reasonable opportunity to investigate it. Open a GitHub security advisory when available, or contact the repository maintainer privately.

Useful reports include:

- affected FlashROM version/commit;
- operating system and Android Platform Tools version;
- device mode and slot layout;
- exact action that bypassed or weakened a safety gate;
- whether an unintended serial/partition was targeted;
- minimal reproduction steps that avoid unnecessary destructive writes.

## Security invariants

Changes should preserve these invariants:

- destructive ADB/Fastboot commands always target an explicit serial;
- multiple connected devices require explicit selection;
- Full-ROM execution rebuilds compatibility/partition metadata and SHA-256 Guard before writes;
- partition metadata and image hashes are revalidated before each write;
- logical partitions require FastbootD and physical partitions require classic Fastboot;
- product mismatch, locked bootloader, active snapshot operations, unknown mode, unsafe ZIP paths and unresolved partitions fail closed;
- protected operations are serialized through the global operation manager;
- user confirmations are checked in the Rust backend, not only in the UI;
- ZIP extraction never executes embedded scripts;
- operation journals are confined to the FlashROM journal directory.

## Out of scope

FlashROM cannot make an inherently unsafe ROM/device combination safe. Bootloader/vendor firmware behavior, custom recovery behavior, unsigned images, hardware failures and user-provided ROM contents remain outside the trust boundary.
