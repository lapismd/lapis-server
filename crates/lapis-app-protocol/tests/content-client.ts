import {
  CONTENT_API_MAJOR,
  type ContentCommand,
  type ContentRequest,
  type ContentReply,
  type ContentCapabilities,
} from "../bindings/protocol.js";

const command = {
  operation: "replace_note",
  vault_id: "admitted-vault-id",
  path: "Inbox/idea.md",
  text: "updated",
  expected_version: "opaque-version",
} satisfies ContentCommand;
const request = {
  api_version: CONTENT_API_MAJOR,
  request_id: "operation-id",
  command,
} satisfies ContentRequest;
// @ts-expect-error An external replace cannot omit its expected version.
const unsafe: ContentCommand = {
  operation: "replace_note",
  vault_id: "vault",
  path: "note.md",
  text: "lost",
};
// @ts-expect-error Runtime admission cannot be supplied by the caller.
const foreign: ContentRequest = { ...request, generation: "foreign" };
// @ts-expect-error No arbitrary filesystem or command-execution operation.
const shell: ContentCommand = { operation: "execute", program: "sh" };
const unavailable: ContentCapabilities = { api_version: 1, operations: [] };

function result(reply: ContentReply) {
  if (reply.outcome.status === "error") {
    return {
      code: reply.outcome.error.code,
      effect: reply.outcome.error.effect,
    };
  }
  const value = reply.outcome.value;
  if (value.kind === "note")
    return { text: value.text, version: value.version };
  return value;
}
void [request, unsafe, foreign, shell, unavailable, result];
