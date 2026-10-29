export const time = "2026-10-27T12:00:00.000Z";
export const uuid = "12345678-1234-4234-8234-123456789abc";
export const token = (id) =>
  `jj-store-agent:${Buffer.from(id).toString("hex")};`;
export const recipient = (id = "codex") => ({
  id,
  token: token(id),
  label: "Captured agent",
});
export function queued(id = "one") {
  return {
    version: 1,
    id,
    state: "queued",
    user: {
      schemaVersion: 3,
      id: `queued-user-${uuid}-${id}`,
      type: "message",
      role: "user",
      text: "Authored café 😀",
      recipients: [recipient()],
      createdAt: time,
    },
    selection: {
      targets: [{ id: "codex", agent: "codex" }],
      runtime: "acp",
      attachments: [],
    },
  };
}
export function comment() {
  return {
    kind: "transcript-comments",
    id: "capture",
    metadata: { submissionId: "capture", retained: true },
    content: JSON.stringify({
      comments: [
        {
          threadId: "thread",
          commentId: "comment",
          text: "Comment",
          author: { id: "owner", name: "Owner" },
          anchor: {
            blockId: "block",
            revision: "rev",
            from: 2,
            to: 4,
            quote: "😀",
            tableContext: { from: 0, to: 3, markdown: "abc" },
          },
        },
      ],
    }),
  };
}
