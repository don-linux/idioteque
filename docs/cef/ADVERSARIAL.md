# CEF adversarial pack

Kickoffs cite this path. Tests first. Production code only if a new test fails.
Workarounds stay if removing them breaks health-check or spawn.

## Workarounds

1. If removing a workaround breaks startup (health-check or spawn), keep it.
2. If Chromium/CEF docs allow better management and startup still works, do that.
3. `/dev/shm` is Chromium’s tmpfs backing store (anonymous file + unlink + mmap), not the `libcef` directory. Do not copy the engine there. Do not force `--disable-dev-shm-usage` when shm is healthy. Official flag only when `/dev/shm` is unusable.
4. Order: startup-repro test → try the documented path → if startup fails, revert and cite why here.

## Linux surfaces

deb / rpm / AppImage. Ubuntu cloud is a lab, not “Linux”. Do not encode AppArmor-only, apt-only, or “64 MiB shm = always disable”.

## Docs

- Contract: [`CONTRACT.md`](CONTRACT.md)
- Runtime: [`../CEF-RUNTIME.md`](../CEF-RUNTIME.md)
- Cloud caveats: [`../../AGENTS.md`](../../AGENTS.md)
- Chromium shm: [`platform_shared_memory_region_posix.cc`](https://chromium.googlesource.com/chromium/src/+/HEAD/base/memory/platform_shared_memory_region_posix.cc), [`GetShmemTempDir`](https://chromium.googlesource.com/chromium/src/+/HEAD/base/files/file_util.h) / [`kDisableDevShmUsage`](https://chromium.googlesource.com/chromium/src/+/HEAD/base/base_switches.h), [crbug/715363](https://crbug.com/715363)
- CEF embed: [issue 3294](https://github.com/chromiumembedded/cef/issues/3294) (native parent), [2804](https://github.com/chromiumembedded/cef/issues/2804) (Ozone/Wayland), [3396](https://github.com/chromiumembedded/cef/issues/3396) (Ozone X11 size)

Lab-only notes (not in git): Project store `internal/cef-implementation-map-adversarial-plan.md`, `internal/cef-module-inventory.md`, `internal/cef-shm-docs.md`. Plan: `artifacts/plans/cef_adversarial_night_4ef5bd80.plan.md`.

## Won’t-test log

| Slice | Reason |
| ----- | ------ |
| | |
