import {
  createNativePluginSecretsHost,
  type PluginCredentialReply,
} from "@lapis-notes/api";
import type { RustVaultClient } from "./vault-client";

export async function preparePluginCredentials(client: RustVaultClient) {
  if (!client.hasPluginCredentialGrants) return undefined;
  const path = `${client.path}/plugin-credentials`;
  const grants = await client.host
    .json<unknown>(path, "GET", undefined, client.signal)
    .catch(() => undefined);
  if (!Array.isArray(grants) || !grants.every((id) => typeof id === "string"))
    return undefined;
  return createNativePluginSecretsHost(
    grants,
    (command) =>
      client.host.json<PluginCredentialReply>(
        path,
        "POST",
        command,
        client.signal,
      ),
    client.signal,
  );
}
