# FlashROM external tools

FlashROM never downloads flashing or ROM-conversion executables at runtime. Operators must provision trusted binaries themselves and can inspect the resolved executable path in the UI before use.

## Android Platform Tools

Resolution order:

1. `FLASHROM_PLATFORM_TOOLS`
2. `tools/platform-tools/`
3. system `PATH`

Required binaries on Windows:

- `adb.exe`
- `fastboot.exe`

Use a current Android SDK Platform Tools release from Google.

## Payload preparation

FlashROM supports guarded preparation of `payload.bin` and OTA ZIP inputs through `payload-dumper-go`.

Resolution order:

1. `FLASHROM_PAYLOAD_DUMPER` — either the executable path or its directory
2. `tools/payload-dumper-go/payload-dumper-go.exe`
3. system `PATH`

FlashROM first requests the payload partition list, intersects it with its own partition allowlist, then extracts only those partitions. It deliberately does not pass `-no-verify`, so payload-dumper-go output verification remains enabled.

Incremental/delta OTAs that require old/base images are not guessed or automatically merged in beta. The preparation step stops with the extractor diagnostic instead.

`payload-dumper-go` may require `xz` to be installed and available on the host depending on the binary/build used.

## Dynamic partition (`super.img`) preparation

FlashROM does not automatically flash raw `super.img` in beta. It unpacks the container first and sends the resulting logical partition images back through the normal compatibility, partition metadata, ordering and SHA-256 Guard pipeline.

Resolution order for `lpunpack`:

1. `FLASHROM_LPUNPACK`
2. `tools/dynamic-partitions/lpunpack.exe`
3. system `PATH`

Resolution order for `simg2img`:

1. `FLASHROM_SIMG2IMG`
2. `tools/dynamic-partitions/simg2img.exe`
3. system `PATH`

Sparse `super.img` requires `simg2img` before `lpunpack`. Unsupported partition images are moved to `_ignored_partitions` and never enter the beta execution plan.

## Supply-chain rule

Do not commit third-party executables to this repository without documenting their source, version, license and cryptographic checksum. Production releases should additionally pin checksums for every bundled external executable.
