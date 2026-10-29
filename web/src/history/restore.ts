import type {
  RepositoryRestoreCommand,
  RepositoryRestoreValue,
} from "../../../crates/lapis-app-protocol/bindings/protocol";
import type {
  FileRestoreRequest,
  RestorePreview,
  RestoreService,
  VaultRestorePreview,
  VaultRestoreRequest,
} from "./services";

/** The existing controller owns pending intents, explicit recovery and replay policy. */
export function createHostedRestore(
  call: (command: RepositoryRestoreCommand) => Promise<RepositoryRestoreValue>,
): RestoreService {
  return {
    async preview(checkpoint, path) {
      const result = await call({ kind: "preview", checkpoint, path });
      if (
        !result ||
        typeof result !== "object" ||
        !("path" in result) ||
        result.checkpoint !== checkpoint ||
        result.path !== path ||
        typeof result.expected !== "string" ||
        !Number.isSafeInteger(result.bytes) ||
        result.bytes < 0 ||
        typeof result.deleted !== "boolean"
      )
        throw new Error("Invalid restore preview");
      return result as RestorePreview;
    },
    apply: (request) => call({ kind: "apply", request }),
    async previewVault(checkpoint) {
      const result = await call({ kind: "preview_vault", checkpoint });
      if (
        !result ||
        typeof result !== "object" ||
        !("added" in result) ||
        result.checkpoint !== checkpoint ||
        typeof result.expected !== "string" ||
        ![result.added, result.modified, result.deleted].every(
          (value) => Number.isSafeInteger(value) && value >= 0,
        )
      )
        throw new Error("Invalid vault restore preview");
      return result as VaultRestorePreview;
    },
    applyVault: (request) => call({ kind: "apply_vault", request }),
    keepCurrent: () => call({ kind: "keep_current" }),
  };
}
