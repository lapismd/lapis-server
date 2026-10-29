import type { AppPluginDistributionOptions } from "@lapis-notes/api";
import type { RustVaultClient } from "./vault-client";

// Hosted release sources, including the official catalog's current mirror.
function official(value: string): boolean {
  try {
    const url = new URL(value);
    return (
      url.protocol === "https:" &&
      !url.port &&
      !url.username &&
      !url.password &&
      !url.search &&
      !url.hash &&
      ((url.hostname === "github.com" &&
        /^\/(?:lapismd\/lapis-plugins|lapis-notes\/releases)\/releases\/download\/[^/]+\/[^/]+\.lapis-plugin$/u.test(
          url.pathname,
        )) ||
        (url.hostname === "registry.lapis.md" &&
          /^\/download\/[A-Za-z0-9-]+\/[^/]+$/u.test(url.pathname)))
    );
  } catch {
    return false;
  }
}
export function createRustPluginDistributionFetch(
  client: RustVaultClient,
): NonNullable<AppPluginDistributionOptions["fetch"]> {
  return (input, init) => {
    client.requireOpen();
    const source = input;
    if (!official(source)) return client.host.fetch(input, init);
    return client.host.response(
      `${client.path}/plugin-bundles/download?source=${encodeURIComponent(source)}`,
      {
        method: init?.method ?? "GET",
        signal: init?.signal,
      },
    );
  };
}
