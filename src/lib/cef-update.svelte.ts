import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { SvelteSet } from "svelte/reactivity";
import {
  incompatibleDetail,
  incompatibleMessage,
  issueUrl,
  updatedMessage,
  type CefUpdateEvent,
} from "$lib/cef-notices";
import type { CefRuntimeInfo } from "$lib/cef-runtime";
import {
  cefUpdateKey,
  drainCefUpdates,
  enqueueCefUpdate,
  shouldDefer,
} from "$lib/cef-update";
import { toasts } from "$lib/toast.svelte";

export class CefUpdates {
  #pending: CefUpdateEvent[] = [];
  #shown = new SvelteSet<string>();
  #generation = 0;
  #unlisten: UnlistenFn | null = null;
  #idiotequeVersion = "dev";
  #hostApiVersion = 0;
  #platform = "unknown";

  async start(): Promise<() => void> {
    const generation = ++this.#generation;
    await this.#loadContext();
    if (generation !== this.#generation) {
      return () => {};
    }
    try {
      const unlisten = await listen<CefUpdateEvent>("cef-update", (event) => {
        this.enqueue(event.payload);
      });
      if (generation !== this.#generation) {
        unlisten();
        return () => {};
      }
      this.#unlisten?.();
      this.#unlisten = unlisten;
      let closed = false;
      return () => {
        if (closed) {
          return;
        }
        closed = true;
        if (this.#unlisten === unlisten) {
          this.#unlisten = null;
        }
        unlisten();
      };
    } catch {
      return () => {};
    }
  }

  enqueue(event: CefUpdateEvent): void {
    if (shouldDefer()) {
      this.#pending = enqueueCefUpdate(this.#pending, event);
      return;
    }
    this.show(event);
  }

  flush(): void {
    if (shouldDefer()) {
      return;
    }
    const drained = drainCefUpdates(this.#pending);
    this.#pending = drained.pending;
    for (const event of drained.events) {
      this.show(event);
    }
  }

  show(event: CefUpdateEvent): void {
    const key = cefUpdateKey(event);
    if (this.#shown.has(key)) {
      return;
    }
    this.#shown.add(key);

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
