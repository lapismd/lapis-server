// Retained Desktop validator/constructor fixture generator; no product dispatch.
// Run from the Notes root with Node --experimental-transform-types and an explicit Desktop checkout.
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { resolve, isAbsolute } from "node:path";
import { pathToFileURL } from "node:url";
const desktop = process.argv[2];
if (!desktop || !isAbsolute(desktop))
  throw Error(
    "Pass the retained Desktop source checkout as an absolute argument",
  );
const revision = "f276120b30395860aebed67362bfb320ded6f22c";
const { validateDispatchIntent, validateDelivery, deliveryEntry } =
  await import(
    pathToFileURL(
      resolve(desktop, "src/lib/ai/conversations/dispatch-intent.ts"),
    ).href
  );
const paths = [
  "src/lib/ai/conversations/dispatch-intent.ts",
  "src/lib/ai/conversations/recipients.ts",
  "src/lib/ai/conversations/validation-fields.ts",
  "src/lib/ai/conversations/types.ts",
  "src/lib/ai/conversations/conversation-dispatches.ts",
  "src/lib/ai/chat/controller/binding.ts",
];
const sources = [];
for (const path of paths) {
  const bytes = await readFile(desktop + "/" + path);
  const frozen = execFileSync("jj", ["file", "show", "-r", revision, path], {
    cwd: desktop,
  });
  if (!bytes.equals(frozen)) throw Error("Retained source changed: " + path);
  sources.push({
    path,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  });
}
const recipient = {
  id: "writer",
  token: "jj-store-agent:777269746572;",
  label: "Captured Writer",
};
const delivery = { id: "delivery:writer", recipient, state: "pending" };
const dispatch = {
  version: 1,
  id: "dispatch-authored",
  userEntryId: "user-authored-café",
  startedAt: "2026-10-27T04:30:00.000Z",
  deliveries: [delivery],
};
const cases = [];
const add = (action, name, input) => {
  try {
    const value = (
      action === "dispatch" ? validateDispatchIntent : validateDelivery
    )(structuredClone(input));
    cases.push({ action, name, input, expected: { ok: true, value } });
  } catch (error) {
    cases.push({
      action,
      name,
      input,
      expected: {
        ok: false,
        error: { name: error.name, message: error.message },
      },
    });
  }
};
const mutate = (action, name, original, fn) => {
  const v = structuredClone(original);
  fn(v);
  add(action, name, v);
};
for (const state of [
  "pending",
  "started",
  "completed",
  "rejected",
  "unknown",
  "not_sent",
  "cancelled",
]) {
  const d = {
    ...structuredClone(delivery),
    state,
    ...(state === "started" ? { agentBindingId: "binding-original" } : {}),
  };
  add("delivery", "delivery-" + state, d);
  add("dispatch", "dispatch-" + state, {
    ...structuredClone(dispatch),
    deliveries: [d],
  });
}
for (const action of ["delivery", "dispatch"]) {
  for (const [name, value] of [
    ["null", null],
    ["array", []],
    ["number", 1],
    ["string", "x"],
  ])
    add(action, action + "-" + name, value);
}
for (const key of ["executable", "vault_id", "environment", "root"]) {
  mutate(
    "delivery",
    "delivery-forged-" + key,
    delivery,
    (v) => (v[key] = "forged"),
  );
  mutate(
    "dispatch",
    "dispatch-forged-" + key,
    dispatch,
    (v) => (v[key] = "forged"),
  );
}
for (const [name, value] of [
  ["missing", undefined],
  ["empty", ""],
  ["number", 7],
  ["too-long", "x".repeat(513)],
  ["utf16-bound", "📝".repeat(256)],
  ["utf16-over", "📝".repeat(257)],
]) {
  for (const action of ["delivery", "dispatch"]) {
    const v = structuredClone(action === "delivery" ? delivery : dispatch);
    if (value === undefined) delete v.id;
    else v.id = value;
    add(action, action + "-id-" + name, v);
  }
}
for (const value of [undefined, null, "", 7, "x".repeat(513)])
  mutate(
    "delivery",
    "delivery-binding-" + String(value).slice(0, 15),
    delivery,
    (v) => {
      v.state = "started";
      if (value !== undefined) v.agentBindingId = value;
    },
  );
mutate(
  "delivery",
  "delivery-unknown-state",
  delivery,
  (v) => (v.state = "sent"),
);
for (const key of ["userEntryId", "startedAt"]) {
  mutate("dispatch", "dispatch-missing-" + key, dispatch, (v) => delete v[key]);
  mutate(
    "dispatch",
    "dispatch-invalid-" + key,
    dispatch,
    (v) => (v[key] = key === "startedAt" ? "not-a-date" : 7),
  );
}
for (const value of [0, 2, "1", null])
  mutate(
    "dispatch",
    "dispatch-version-" + String(value),
    dispatch,
    (v) => (v.version = value),
  );
for (const value of [
  null,
  {},
  [],
  Array.from({ length: 5 }, (_, i) => ({
    ...structuredClone(delivery),
    id: "delivery-" + i,
  })),
])
  mutate(
    "dispatch",
    "dispatch-deliveries-" +
      (Array.isArray(value) ? value.length : String(value)),
    dispatch,
    (v) => (v.deliveries = value),
  );
mutate(
  "dispatch",
  "dispatch-four",
  dispatch,
  (v) =>
    (v.deliveries = Array.from({ length: 4 }, (_, i) => ({
      id: "delivery-" + i,
      recipient: {
        id: "writer" + i,
        token:
          "jj-store-agent:" + Buffer.from("writer" + i).toString("hex") + ";",
        label: "Same label",
      },
      state: "pending",
    }))),
);
mutate("dispatch", "dispatch-duplicate-delivery", dispatch, (v) =>
  v.deliveries.push({
    ...structuredClone(delivery),
    recipient: {
      id: "reviewer",
      token: "jj-store-agent:7265766965776572;",
      label: "Reviewer",
    },
  }),
);
mutate("dispatch", "dispatch-duplicate-recipient", dispatch, (v) =>
  v.deliveries.push({ ...structuredClone(delivery), id: "another-delivery" }),
);
for (const [name, fn] of [
  ["bad-token", (r) => (r.token = "jj-store-agent:ff;")],
  ["mismatch", (r) => (r.id = "reviewer")],
  ["empty-label", (r) => (r.label = " ")],
  ["control-label", (r) => (r.label = "bad\nlabel")],
  ["long-label", (r) => (r.label = "x".repeat(81))],
  ["extra", (r) => (r.executable = "/tmp/agent")],
])
  mutate("delivery", "delivery-recipient-" + name, delivery, (v) =>
    fn(v.recipient),
  );
const ActualDate = Date;
globalThis.Date = class extends ActualDate {
  constructor(...args) {
    super(...(args.length ? args : ["2026-10-27T04:30:00.000Z"]));
  }
};
const entries = [
  "pending",
  "started",
  "completed",
  "rejected",
  "unknown",
  "not_sent",
  "cancelled",
].map((state) => {
  const value = {
    ...structuredClone(delivery),
    state,
    ...(state !== "pending" ? { agentBindingId: "binding-original" } : {}),
  };
  return {
    dispatch: structuredClone(dispatch),
    delivery: value,
    entry: deliveryEntry(dispatch, value),
  };
});
globalThis.Date = ActualDate;
const result = {
  schema: 1,
  desktopRevision: revision,
  sources,
  cases,
  entries,
};
await mkdir("crates/lapis-app-core/tests/fixtures", { recursive: true });
const bytes = JSON.stringify(result, null, 2) + "\n";
await writeFile(
  "crates/lapis-app-core/tests/fixtures/participant-capture-v1.json",
  bytes,
);
console.log(
  JSON.stringify({
    cases: cases.length,
    entries: entries.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    sources,
  }),
);
