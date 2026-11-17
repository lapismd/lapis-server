import type { SkillManagementHost } from "@lapis-notes/api";
import type { OperationReply } from "../../../../crates/lapis-app-protocol/bindings/protocol";
import type { RustVaultClient } from "./vault-client";

/** Authenticated native owner; renderer detach never replays accepted writes. */
export function createRustSkillManagement(
  client: RustVaultClient,
): SkillManagementHost {
  return {
    async manage(action, skill) {
      client.requireOpen();
      const reply = await client.host.json<OperationReply>(
        `${client.path}/plugins/skills/operations/${action}`,
        "POST",
        {
          protocol_version: 1,
          request_id: crypto.randomUUID(),
          version: 1,
          input: skill ? { skill } : {},
        },
      );
      if (reply.outcome.status === "error")
        throw new Error(reply.outcome.error.message);
      return reply.outcome.value as Awaited<
        ReturnType<SkillManagementHost["manage"]>
      >;
    },
  };
}
