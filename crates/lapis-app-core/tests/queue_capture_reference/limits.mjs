import { comment } from "./fixtures.mjs";
export function appendLimits(change) {
  change("retained-state-coercion", (q) => {
    q.state = ["queued"];
  });
  change("retained-runtime-coercion", (q) => {
    q.selection.runtime = ["acp"];
  });
  for (const n of [16, 17])
    change(`comment-blocks-${n}`, (q) => {
      q.user.contextBlocks = Array.from({ length: n }, (_, i) => ({
        ...comment(),
        id: String(i),
        metadata: { submissionId: String(i) },
      }));
    });
  for (const n of [64, 65])
    change(`captured-comments-${n}`, (q) => {
      const c = comment(),
        input = JSON.parse(c.content);
      input.comments = Array.from({ length: n }, (_, i) => ({
        ...input.comments[0],
        commentId: String(i),
      }));
      c.content = JSON.stringify(input);
      q.user.contextBlocks = [c];
    });
  for (const n of [65536, 65537])
    change(`comment-bytes-${n}`, (q) => {
      const c = comment();
      c.metadata.padding = "";
      c.metadata.padding = "x".repeat(
        n - Buffer.byteLength(JSON.stringify([c])),
      );
      q.user.contextBlocks = [c];
    });
  for (const [name, mutate] of [
    [
      "fractional-range",
      (c) => {
        c.anchor.from = 0.5;
      },
    ],
    [
      "unsafe-range",
      (c) => {
        c.anchor.to = 9007199254740992;
      },
    ],
    [
      "negative-range",
      (c) => {
        c.anchor.from = -1;
      },
    ],
    [
      "table-utf16-valid",
      (c) => {
        c.anchor.tableContext = { from: 0, to: 2, markdown: "😀" };
      },
    ],
    [
      "table-utf16-over",
      (c) => {
        c.anchor.tableContext = { from: 0, to: 3, markdown: "😀" };
      },
    ],
    [
      "comment-id-utf16-valid",
      (c) => {
        c.commentId = "😀".repeat(256);
      },
    ],
    [
      "comment-id-utf16-over",
      (c) => {
        c.commentId = "😀".repeat(257);
      },
    ],
    [
      "comment-author-del",
      (c) => {
        c.author.name = "Owner\x7f";
      },
    ],
    [
      "comment-author-control",
      (c) => {
        c.author.name = "Owner\n";
      },
    ],
    [
      "comment-extra-preserved",
      (c) => {
        c.extra = { retained: true };
      },
    ],
  ])
    change(name, (q) => {
      const block = comment(),
        content = JSON.parse(block.content);
      mutate(content.comments[0]);
      block.content = JSON.stringify(content);
      q.user.contextBlocks = [block];
    });
  for (const n of [256, 257])
    change(`model-utf16-${n}`, (q) => {
      q.selection.targets[0].model = { provider: "😀".repeat(n), model: "m" };
    });
  for (const n of [8192, 8193])
    change(`instructions-utf8-${n}`, (q) => {
      q.selection.targets[0].agentProfile = { instructions: "😀".repeat(n) };
    });
  for (const n of [128, 129])
    change(`mode-utf16-${n}`, (q) => {
      q.selection.targets[0].executionMode = "😀".repeat(n);
    });
  change("authored-id-retained-no-512-bound", (q) => {
    q.user.id = "x".repeat(513);
  });
  for (const profileId of [
    "",
    "x".repeat(128),
    "x".repeat(129),
    "invalid/config",
    null,
  ])
    change(
      `provider-profile-${String(profileId).slice(0, 8)}-${String(profileId).length}`,
      (q) => {
        q.selection.targets[0].agentProfile = { providerProfileId: profileId };
      },
    );
}
