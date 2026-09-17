import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  incompatibleDetail,
  incompatibleMessage,
  issueUrl,
  updatedMessage,
  type CefUpdateEvent,
} from "$lib/cef-notices";
import type { CefRuntimeInfo } from "$lib/cef-runtime";
import { drainCefUpdates, enqueueCefUpdate, shouldDefer } from "$lib/cef-update";
import { toasts } from "$lib/toast.svelte";
import { surface } from "$lib/workspace-surface.svelte";

class CefUpdates {
  #pending: CefUpdateEvent[] = [];
  #idiotequeVersion = "dev";
  #hostApiVersion = 0;
  #platform = "unknown";

  async start(): Promise<() => void> {
    await this.#loadContext();
    try {
      return await listen<CefUpdateEvent>("cef-update", (event) => {
        this.enqueue(event.payload);
      });
    } catch {
      return () => {};
    }
  }

  enqueue(event: CefUpdateEvent): void {
    if (shouldDefer(surface.current)) {
      this.#pending = enqueueCefUpdate(this.#pending, event);
      return;
    }
    this.show(event);
  }

  flush(): void {
    const drained = drainCefUpdates(this.#pending);
    this.#pending = drained.pending;
    for (const event of drained.events) {
      this.show(event);
    }
  }

  show(event: CefUpdateEvent): void {
    if (event.kind === "updated") {
      toasts.successLong(updatedMessage(event));
      return;
    }
    toasts.notice(incompatibleMessage(event), {
      detail: incompatibleDetail(event),
      action: {
        label: "Abrir issue",
        href: issueUrl(event, {
          idiotequeVersion: this.#idiotequeVersion,
          hostApiVersion: this.#hostApiVersion,
          platform: this.#platform,
        }),
      },
    });
  }

  async #loadContext(): Promise<void> {
    try {
      this.#idiotequeVersion = await getVersion();
    } catch {
      this.#idiotequeVersion = "dev";
    }

    try {
      const info = await invoke<CefRuntimeInfo>("cef_runtime_info");
      this.#hostApiVersion = info.hostApiVersion;
      this.#platform = info.platform;
    } catch {
      this.#hostApiVersion = 0;
      this.#platform = "unknown";
    }
  }
}

export const cefUpdates = new CefUpdates();
