import { redirect } from "@sveltejs/kit";
import { ROUTES } from "$lib/app-routes";
import { workspace } from "$lib/workspace.svelte";

export function load() {
  if (workspace.root === null) {
    redirect(302, ROUTES.home);
  }
}
