import type { NativeEditingIntents } from "@lapis-notes/api/native-editing-intents";
import { RustHostError } from "./client";
import type { RustVaultClient } from "./vault-client";

/** Same admitted owner as writes and Tasks; no second lifetime or replay path. */
export function createRustNativeEditingIntents(
  client: RustVaultClient,
): NativeEditingIntents {
  return {
    async call(command) {
      const value = await client.execute({ kind: "editing_intents", command });
      if (value.kind !== "editing_intents")
        throw new Error("Invalid native editing-intent response");
      return value.value;
    },
    async read(path) {
      try {
        const value = await client.execute({ kind: "read_text", path });
        if (value.kind !== "text")
          throw new Error("Invalid native editing snapshot");
        return value.snapshot;
      } catch (error) {
        if (error instanceof RustHostError && error.code === "NOT_FOUND")
          return null;
        throw error;
      }
    },
  };
}
