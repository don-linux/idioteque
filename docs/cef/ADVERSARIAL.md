# CEF adversarial pack

Kickoffs cite this path. Tests first. Production code only if a new test fails.
Workarounds stay if removing them breaks health-check or spawn.

## Workarounds

1. If removing a workaround breaks startup (health-check or spawn), keep it.
2. If Chromium/CEF docs allow better management and startup still works, do that.
3. `/dev/shm` is Chromium’s tmpfs backing store (anonymous file + unlink + mmap), not the `libcef` directory. Do not copy the engine there. Do not force `--disable-dev-shm-usage` when shm is healthy. Official flag only when `/dev/shm` is unusable.
4. Order: startup-repro test → try the documented path → if startup fails, revert and cite why here.

### Kept: shm probe 128 MiB + `CacheDir` (`cursor/cef-test-shm-911c` @ `110b55b`)

Tried the two official Chromium levels only (`/dev/shm` default, `--disable-dev-shm-usage` → `$TMPDIR`). Startup-repro tests fail without the extras:

- **128 MiB `fallocate` probe** — ChromeDriver’s `access(W_OK|X_OK)` accepts Docker’s 64 MiB `/dev/shm`; Chromium then dies with `ENOSPC` / `TransferBuffer::Initialize() failed` (crbug/715363). The probe is not “cloud = always disable”: a desktop/deb/rpm/AppImage with 1–8 GiB shm still chooses `DevShm`.
- **`CacheDir` + `TMPDIR=<cache>/shm`** — official level 2 still uses `GetTempDir()`. If that tmpfs is also unusable (`EDQUOT` / usrquota, systemd ≥ 258, any distro), the flag alone points at the same broken temp. Third fallback stays on disk cache.

`/dev/shm` stays first. Flag only when the probe says shm is unusable. Do not copy `libcef` into `/dev/shm`.

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
| shm (drop 128 MiB probe) | Startup-repro: 64 MiB shm passes `access()` then Chromium OOMs. Kept. |
| shm (drop `CacheDir`) | Startup-repro: official flag + broken `$TMPDIR` still cannot reserve shm. Kept. |
