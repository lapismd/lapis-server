import { queued, comment, recipient, uuid } from "./fixtures.mjs";
import { appendBoundaries } from "./boundaries.mjs";
import { appendLimits } from "./limits.mjs";
export { queued, time, uuid } from "./fixtures.mjs";
export function cases() {
  const result = [];
  const add = (name, action, input) => result.push({ name, action, input });
  const change = (name, mutate) => {
    const q = queued();
    mutate(q);
    add(name, "message", q);
  };
  add("minimal-omissions", "message", queued());
  change("all-retained-fields", (q) => {
    q.state = "dispatching";
    q.dispatchId = uuid;
    q.user.provenance = { originClass: "owner", sourceKind: "user-message" };
    q.user.contextBlocks = [comment()];
    q.selection.thinking = "high";
    Object.assign(q.selection.targets[0], {
      thinking: "default",
      model: { provider: "codex", model: "model" },
      executionMode: "read-only",
      agentProfile: {
        name: "Captured",
        description: "Description",
        instructions: "Exact instructions\n😀",
        providerProfileId: "opaque-config",
      },
    });
    q.selection.attachments = [
      " Notes\\café.md ",
      ".lapis/visible-to-retained-validator",
      "./a.md/",
    ];
  });
  change("authored-id-not-dispatch-uuid", (q) => {
    q.user.id = `queued-user-${uuid}`;
    q.state = "dispatching";
    q.dispatchId = "retained-non-uuid-dispatch";
  });
  for (const thinking of [
    null,
    "default",
    "off",
    "low",
    "medium",
    "high",
    "invalid",
    1,
  ])
    change(`thinking-${thinking}`, (q) => {
      q.selection.thinking = thinking;
      q.selection.targets[0].thinking = thinking;
    });
  for (const runtime of [
    "acp",
    "codex-native",
    "opencode-native",
    "fake",
    "other",
    null,
  ])
    change(`runtime-${runtime}`, (q) => {
      q.selection.runtime = runtime;
    });
  change("four-recipients", (q) => {
    q.user.recipients = ["codex", "cursor", "opencode", "local"].map(recipient);
    q.selection.targets = q.user.recipients.map((r) => ({
      id: r.id,
      agent: r.id,
    }));
  });
  change("five-recipients", (q) => {
    q.user.recipients = ["a", "b", "c", "d", "e"].map(recipient);
  });
  for (const [name, mutate] of [
    [
      "unknown-queue-field",
      (q) => {
        q.executable = "/bin/tool";
      },
    ],
    [
      "unknown-user-field",
      (q) => {
        q.user.nativeBinding = "binding";
      },
    ],
    [
      "unknown-selection-field",
      (q) => {
        q.selection.credential = "forbidden";
      },
    ],
    [
      "unknown-target-field",
      (q) => {
        q.selection.targets[0].environment = {};
      },
    ],
    [
      "unknown-model-field",
      (q) => {
        q.selection.targets[0].model = {
          provider: "p",
          model: "m",
          displayName: "extra",
        };
      },
    ],
    [
      "unknown-profile-field",
      (q) => {
        q.selection.targets[0].agentProfile = { token: "forbidden" };
      },
    ],
    [
      "unknown-recipient-field",
      (q) => {
        q.user.recipients[0].publicKey = "key";
      },
    ],
    [
      "unknown-provenance-field",
      (q) => {
        q.user.provenance = {
          originClass: "owner",
          sourceKind: "user-message",
          other: true,
        };
      },
    ],
    [
      "wrong-provenance",
      (q) => {
        q.user.provenance = {
          originClass: "agent",
          sourceKind: "user-message",
        };
      },
    ],
    [
      "queued-with-dispatch",
      (q) => {
        q.dispatchId = uuid;
      },
    ],
    [
      "dispatching-missing-dispatch",
      (q) => {
        q.state = "dispatching";
      },
    ],
    [
      "dispatching-null-dispatch",
      (q) => {
        q.state = "dispatching";
        q.dispatchId = null;
      },
    ],
    [
      "invalid-state",
      (q) => {
        q.state = "paused";
      },
    ],
    [
      "duplicate-recipients",
      (q) => {
        q.user.recipients.push(recipient());
      },
    ],
    [
      "reordered-target",
      (q) => {
        q.selection.targets[0].id = "other";
      },
    ],
    [
      "missing-target",
      (q) => {
        q.selection.targets = [];
      },
    ],
    [
      "invalid-agent",
      (q) => {
        q.selection.targets[0].agent = "other";
      },
    ],
    [
      "empty-authored",
      (q) => {
        q.user.text = " \n";
      },
    ],
    [
      "comment-only",
      (q) => {
        q.user.text = "";
        q.user.contextBlocks = [comment()];
      },
    ],
    [
      "null-comments",
      (q) => {
        q.user.contextBlocks = null;
      },
    ],
    [
      "null-profile",
      (q) => {
        q.selection.targets[0].agentProfile = null;
      },
    ],
    [
      "null-model",
      (q) => {
        q.selection.targets[0].model = null;
      },
    ],
    [
      "null-mode",
      (q) => {
        q.selection.targets[0].executionMode = null;
      },
    ],
  ])
    change(name, mutate);
  appendBoundaries(add, change, result);
  appendLimits(change);
  return result;
}
