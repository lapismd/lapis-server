import {
  assertSupportedPluginAssetPath,
  createWebPluginAssetUrl,
  getInstalledPluginAssetFingerprintSha256,
  normalizePluginAssetRelativePath,
  PluginAssetServerError,
  type InstalledPluginRecord,
  type PluginAssetServer,
} from "@lapis-notes/api/plugin-assets";
import type { RustVaultClient } from "./vault-client";

/** URL construction only. The admitted native reader verifies every served byte. */
export function createRustPluginAssetServer(
  client: RustVaultClient,
  adapter: { read(path: string): Promise<string> },
): PluginAssetServer {
  return {
    async getPluginAssetUrl(request) {
      client.requireOpen();
      const state = JSON.parse(
        await adapter.read(".lapis/installed-plugins.json"),
      ) as {
        plugins?: Record<string, InstalledPluginRecord>;
      };
      const record =
        state.plugins && Object.hasOwn(state.plugins, request.pluginId)
          ? state.plugins[request.pluginId]
          : undefined;
      if (!record || record.pluginId !== request.pluginId)
        throw new PluginAssetServerError(
          "asset-not-installed",
          `Plugin ${request.pluginId} is not installed.`,
        );
      if (record.installedVersion !== request.version)
        throw new PluginAssetServerError(
          "asset-version-mismatch",
          `Plugin ${request.pluginId} installation changed.`,
        );
      const path = normalizePluginAssetRelativePath(request.relativePath);
      assertSupportedPluginAssetPath(path);
      const route = createWebPluginAssetUrl({
        vaultId: client.descriptor.client_id,
        pluginId: request.pluginId,
        version: record.installedVersion,
        sha256: await getInstalledPluginAssetFingerprintSha256(record),
        path,
      });
      return `${client.host.baseUrl.replace(/\/$/u, "")}${route}`;
    },
  };
}
