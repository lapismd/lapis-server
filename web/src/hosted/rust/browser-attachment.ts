import type { OfflineWorkspace } from "../../offline/workspace";
import type {
  ClientDescriptor,
  EditingIntentCommand,
} from "../../../../crates/lapis-app-protocol/bindings/protocol";
import type { RustHttpClient } from "./client";
import { RustVaultClient } from "./vault-client";

const prefix = "lapis-server:editor-attachment:";
const uuid = /^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i;

type RetainedAttachment = {
  account: string;
  descriptor: ClientDescriptor;
  editors: string[];
};

function browserStorage() {
  try {
    if (typeof navigator === "undefined" || !navigator.locks) return;
    return globalThis.localStorage;
  } catch {
    // An inaccessible browser profile retains ordinary native recovery.
  }
}

function entry(storage: Storage, key: string): RetainedAttachment | undefined {
  try {
    const result = JSON.parse(
      storage.getItem(key) ?? "null",
    ) as RetainedAttachment | null;
    if (
      result &&
      typeof result.account === "string" &&
      uuid.test(result.descriptor.client_id) &&
      uuid.test(result.descriptor.vault_id) &&
      uuid.test(result.descriptor.generation) &&
      Array.isArray(result.editors) &&
      result.editors.length <= 512 &&
      result.editors.every((id) => uuid.test(id))
    ) {
      return result;
    }
  } catch {
    // Ignore an invalid or unavailable entry.
  }
}

async function call(client: RustVaultClient, command: EditingIntentCommand) {
  const result = await client.execute({ kind: "editing_intents", command });
  if (result.kind !== "editing_intents")
    throw Error("Invalid editing intent reply");
  return result.value;
}

async function retireCleanEditors(
  client: RustVaultClient,
  editors: readonly string[],
) {
  const pending: string[] = [];
  for (const editor_id of editors) {
    try {
      const reply = await call(client, { kind: "inspect", editor_id });
      if (reply.kind !== "state") continue;
      const { path, revision, generation, dirty, active } = reply.state;
      if (dirty) {
        pending.push(editor_id);
        continue;
      }
      if (active) {
        const snapshot = await client.execute({ kind: "read_text", path });
        if (snapshot.kind !== "text") throw Error("Invalid editing snapshot");
        await call(client, {
          kind: "yield",
          editor_id,
          revision,
          generation,
          expected: { kind: "version", version: snapshot.snapshot.version },
        });
      }
      await call(client, { kind: "retire", editor_id, generation });
    } catch {
      pending.push(editor_id);
    }
  }
  return pending;
}

/** Only the former browser owner can retire its clean editing intents. */
export async function recoverBrowserAttachments(
  host: RustHttpClient,
  current: RustVaultClient,
  account: string,
): Promise<void> {
  const storage = browserStorage();
  if (!storage) return;
  const keys = Array.from({ length: storage.length }, (_, index) =>
    storage.key(index),
  ).filter((key): key is string => !!key?.startsWith(prefix));
  for (const key of keys) {
    const previous = entry(storage, key);
    if (
      !previous ||
      previous.account !== account ||
      previous.descriptor.vault_id !== current.descriptor.vault_id
    )
      continue;
    await navigator.locks
      .request(key, { ifAvailable: true }, async (lock) => {
        if (!lock) return;
        const old = RustVaultClient.admitted(
          host,
          previous.descriptor,
          current.descriptor.vault_id,
          false,
        );
        const live = await old
          .revalidate(AbortSignal.timeout(3500))
          .catch(() => false);
        if (live) {
          previous.editors = await retireCleanEditors(old, previous.editors);
          if (!previous.editors.length) await old.close();
        } else if (previous.editors.length) {
          const reply = await call(current, { kind: "list" });
          if (reply.kind !== "states") return;
          const retained = reply.states.filter(({ editor_id }) =>
            previous.editors.includes(editor_id),
          );
          for (const {
            editor_id,
            detached,
            dirty,
            generation,
            revision,
          } of retained) {
            if (detached && !dirty)
              await call(current, {
                kind: "discard",
                editor_id,
                generation,
                revision,
              });
          }
          previous.editors = retained
            .filter(({ dirty, detached }) => dirty || !detached)
            .map(({ editor_id }) => editor_id);
        }
        if (previous.editors.length)
          storage.setItem(key, JSON.stringify(previous));
        else storage.removeItem(key);
      })
      .catch(() => {});
  }
}

/** Hold a browser lock until disposal so another tab cannot retire a live writer. */
export async function trackBrowserAttachment(
  client: RustVaultClient,
  account: string,
  offline: OfflineWorkspace,
) {
  const storage = browserStorage();
  if (!storage) return;
  const key = prefix + client.descriptor.client_id;
  const retained: RetainedAttachment = {
    account,
    descriptor: client.descriptor,
    editors: [],
  };
  let unlock = () => {};
  await new Promise<void>((resolve) => {
    void navigator.locks
      .request(
        key,
        () =>
          new Promise<void>((release) => {
            unlock = release;
            resolve();
          }),
      )
      .catch(resolve);
  });
  function persist() {
    try {
      if (retained.editors.length && offline.current.grant?.account === account)
        storage?.setItem(key, JSON.stringify(retained));
      else storage?.removeItem(key);
    } catch {
      // Durable tracking is optional. Native recovery remains authoritative.
    }
  }
  const stop = offline.subscribe(persist);
  return {
    acquire(editor: string) {
      if (!retained.editors.includes(editor) && retained.editors.length < 512)
        retained.editors.push(editor);
      persist();
    },
    retire(editor: string) {
      retained.editors = retained.editors.filter((id) => id !== editor);
      persist();
    },
    dispose() {
      stop();
      unlock();
    },
  };
}
