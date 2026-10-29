import { queued, comment, token } from "./fixtures.mjs";
export function appendBoundaries(add, change, result) {
  for (const [field, values] of Object.entries({
    id: ["", "😀".repeat(256), "😀".repeat(257)],
    text: [
      "x".repeat(32768),
      "x".repeat(32769),
      "😀".repeat(8192),
      "😀".repeat(8193),
    ],
    createdAt: [
      "invalid",
      "2026-01-01",
      "Tue, 27 Oct 2026 12:00:00 GMT",
      "2026-02-30T00:00:00Z",
    ],
  }))
    for (const [i, value] of values.entries())
      change(`${field}-boundary-${i}`, (q) => {
        if (field === "id") q.id = value;
        else q.user[field] = value;
      });
  for (const [i, value] of [
    "",
    "😀".repeat(80),
    "😀".repeat(81),
    "bad\nlabel",
    "bad\u2028label",
  ].entries())
    change(`label-${i}`, (q) => {
      q.user.recipients[0].label = value;
    });
  for (const [i, value] of [
    "jj-store-agent:ff;",
    "jj-store-agent:636f646578;extra",
    "jj-store-agent:636F646578;",
    token("x".repeat(65)),
    token("😀".repeat(32)),
    token("😀".repeat(33)),
    token("\uFEFFcodex"),
  ].entries())
    change(`token-${i}`, (q) => {
      q.user.recipients[0].token = value;
      q.user.recipients[0].id = [3, 4, 5, 6].includes(i)
        ? ["x".repeat(65), "😀".repeat(32), "😀".repeat(33), "\uFEFFcodex"][
            i - 3
          ]
        : "codex";
      q.selection.targets[0].id = q.user.recipients[0].id;
    });
  for (const [i, path] of [
    "../escape",
    "/absolute",
    "C:\\absolute",
    "https://host/x",
    "a//b",
    ".",
    "a/./b",
    "a\\b",
    "a:b",
    "a\0b",
    "😀".repeat(1024),
    "😀".repeat(1025),
  ].entries())
    change(`path-${i}`, (q) => {
      q.selection.attachments = [path];
    });
  for (const count of [8, 9])
    change(`attachments-${count}`, (q) => {
      q.selection.attachments = Array(count).fill("same.md");
    });
  for (const [field, limit] of [
    ["name", 80],
    ["description", 280],
    ["instructions", 32768],
  ])
    for (const n of [limit, limit + 1])
      change(`profile-${field}-${n}`, (q) => {
        q.selection.targets[0].agentProfile = { [field]: "x".repeat(n) };
      });
  for (const mode of [" ", "x".repeat(256), "x".repeat(257), "bad\x7f"])
    change(`mode-${result.length}`, (q) => {
      q.selection.targets[0].executionMode = mode;
    });
  for (const mutate of [
    (c) => {
      c.metadata.submissionId = "wrong";
    },
    (c) => {
      c.content = "not json";
    },
    (c) => {
      const v = JSON.parse(c.content);
      v.comments[0].anchor.to = 3;
      c.content = JSON.stringify(v);
    },
    (c) => {
      const v = JSON.parse(c.content);
      v.comments.push(v.comments[0]);
      c.content = JSON.stringify(v);
    },
    (c) => {
      c.extra = "retained";
    },
  ])
    change(`comment-${result.length}`, (q) => {
      const c = comment();
      mutate(c);
      q.user.contextBlocks = [c];
    });
  add("empty-list", "queue", []);
  add("missing-list", "queue", undefined);
  add("null-list", "queue", null);
  for (const n of [16, 17])
    add(
      `items-${n}`,
      "queue",
      Array.from({ length: n }, (_, i) => queued(String(i))),
    );
  add("duplicate-queue-id", "queue", [
    queued(),
    { ...queued("two"), id: "one" },
  ]);
  const dup = queued("two");
  dup.user.id = queued().user.id;
  add("duplicate-user-id", "queue", [queued(), dup]);
  for (const delta of [0, 1]) {
    const qs = Array.from({ length: 4 }, (_, i) => queued(String(i)));
    for (const q of qs) q.user.text = "x".repeat(32000);
    const remaining = 131072 - Buffer.byteLength(JSON.stringify(qs));
    qs[0].selection.targets[0].agentProfile = {
      instructions: "x".repeat(remaining - 35 + delta),
    };
    add(`encoded-capacity-${delta}`, "queue", qs);
  }
}
