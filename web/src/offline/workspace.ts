import { writable } from "svelte/store";
import {
  OfflineDatabase,
  request,
  type OfflineGrant,
  type OfflineVault,
  type SavedNote,
} from "./database";

const maxNotes = 50;
const maxTextLength = 500_000;
const maxDelay = 2_147_483_647;
const grantKey = "trusted-device";
type Snapshot = { grant?: OfflineGrant; notes: SavedNote[] };

export type NoteVersion = { text: string; etag: string };
export type DraftRemote = {
  read(path: string): Promise<NoteVersion | undefined>;
  write(path: string, text: string, etag: string): Promise<void>;
};

export function isOfflineMarkdown(path: string): boolean {
  return (
    /\.(?:md|markdown)$/i.test(path) &&
    !path.includes("\\") &&
    path.split("/").every((part) => part !== "" && !part.startsWith("."))
  );
}

function validGrant(grant: OfflineGrant | undefined): grant is OfflineGrant {
  return !!grant && Date.now() < grant.expiresAt;
}

export class OfflineWorkspace {
  #snapshot: Snapshot = { notes: [] };
  #store = writable(this.#snapshot);
  #timer?: ReturnType<typeof setTimeout>;
  #updates?: BroadcastChannel;
  #restricted = false;

  readonly subscribe = this.#store.subscribe;

  constructor(private readonly database = new OfflineDatabase()) {}

  get current() {
    return validGrant(this.#snapshot.grant) && !this.#restricted
      ? this.#snapshot
      : { notes: [] };
  }

  start(): () => void {
    if ("BroadcastChannel" in globalThis) {
      this.#updates = new BroadcastChannel("lapis-server-offline");
      this.#updates.onmessage = () => this.#refreshAfterBroadcast();
    }
    return () => {
      clearTimeout(this.#timer);
      this.#updates?.close();
      this.#updates = undefined;
    };
  }

  #refreshAfterBroadcast(): void {
    // Hide stale data immediately while verifying the shared grant.
    this.#publish({ notes: [] });
    void this.refresh().catch(() => {});
  }

  restrict(): void {
    this.#restricted = true;
    this.#publish({ notes: [] });
  }

  async authorize(
    account: string,
    expiresAt: number | undefined,
    vaults: OfflineVault[],
  ) {
    this.#restricted = false;
    await this.database.transact("readwrite", async (settings, notes) => {
      const existing = await request<OfflineGrant | undefined>(
        settings.get(grantKey),
      );
      if (!existing) return;
      if (existing.account !== account) {
        settings.delete(grantKey);
        notes.clear();
        return;
      }
      const allowed = new Set(vaults.map((vault) => vault.id));
      for (const key of await request<IDBValidKey[]>(notes.getAllKeys())) {
        if (Array.isArray(key) && !allowed.has(String(key[1])))
          notes.delete(key);
      }
      settings.put(
        {
          account,
          expiresAt: expiresAt ?? 0,
          vaults,
          selection:
            existing.selection && allowed.has(existing.selection.vault)
              ? existing.selection
              : undefined,
        } satisfies OfflineGrant,
        grantKey,
      );
    });
    await this.refresh();
  }

  async enable(
    account: string,
    expiresAt: number,
    vaults: OfflineVault[],
    vault: string,
  ) {
    if (expiresAt <= Date.now() || !vaults.some((entry) => entry.id === vault))
      throw Error("Open an authenticated vault before enabling offline data.");
    this.#restricted = false;
    await this.database.transact("readwrite", async (settings, notes) => {
      const existing = await request<OfflineGrant | undefined>(
        settings.get(grantKey),
      );
      if (existing && existing.account !== account) notes.clear();
      settings.put(
        {
          account,
          expiresAt,
          vaults,
          selection: { vault },
        } satisfies OfflineGrant,
        grantKey,
      );
    });
    await this.refresh();
  }

  async clear(): Promise<void> {
    this.restrict();
    await this.database.transact("readwrite", async (settings, notes) => {
      settings.delete(grantKey);
      notes.clear();
    });
    this.#updates?.postMessage("cleared");
  }

  async refresh(): Promise<void> {
    const snapshot = await this.database.transact(
      "readonly",
      async (settings, notes) => {
        const grant = await request<OfflineGrant | undefined>(
          settings.get(grantKey),
        );
        if (!validGrant(grant) || this.#restricted) return { notes: [] };
        const rows: SavedNote[] = grant.selection
          ? await request(
              notes
                .index("vault")
                .getAll([grant.account, grant.selection.vault]),
            )
          : [];
        return { grant, notes: rows.sort((a, b) => b.opened - a.opened) };
      },
    );
    this.#publish(snapshot);
  }

  async select(account: string, vault: string, path?: string): Promise<void> {
    await this.#admitted(account, vault, async (grant, settings) => {
      const selection = {
        vault,
        ...(path && isOfflineMarkdown(path) ? { path } : {}),
      };
      settings.put({ ...grant, selection }, grantKey);
    });
    await this.refresh();
  }

  async cache(
    account: string,
    vault: string,
    path: string,
    value: NoteVersion,
  ): Promise<void> {
    if (
      !isOfflineMarkdown(path) ||
      value.text.length > maxTextLength ||
      !/^"[a-f\d]{64}"$/i.test(value.etag)
    )
      return;
    await this.#admitted(account, vault, async (_grant, _settings, store) => {
      const previous = await request<SavedNote | undefined>(
        store.get([account, vault, path]),
      );
      store.put({
        ...previous,
        account,
        vault,
        path,
        ...value,
        opened: Date.now(),
      } satisfies SavedNote);
      const all: SavedNote[] = await request(store.getAll());
      const disposable = all
        .filter((note) => !note.draft)
        .sort((a, b) => b.opened - a.opened);
      for (const excess of disposable.slice(maxNotes))
        store.delete([excess.account, excess.vault, excess.path]);
    });
    await this.refresh();
  }

  getNote(
    account: string,
    vault: string,
    path: string,
  ): Promise<SavedNote | undefined> {
    return this.#admitted(account, vault, (_grant, _settings, store) =>
      request<SavedNote | undefined>(store.get([account, vault, path])),
    );
  }

  async stage(
    note: SavedNote,
    text: string,
    baseEtag = note.etag,
  ): Promise<SavedNote> {
    if (text.length > maxTextLength)
      throw Error("This note is too large to save as a local draft.");
    const saved = await this.#admitted(
      note.account,
      note.vault,
      async (_grant, _settings, store) => {
        const existing = await request<SavedNote | undefined>(
          store.get([note.account, note.vault, note.path]),
        );
        if (!existing || existing.draft?.revision !== note.draft?.revision)
          throw Error(
            "The local draft changed in another tab. Reopen it before saving.",
          );
        if (text === (existing.draft?.text ?? note.text)) return existing;
        const updated = {
          ...existing,
          draft: {
            text,
            baseEtag: note.draft?.baseEtag ?? baseEtag,
            revision: crypto.randomUUID(),
          },
        };
        store.put(updated);
        return updated;
      },
    );
    if (!saved)
      throw Error(
        "Offline access is locked. Your changes are still in the editor.",
      );
    if (
      this.current.grant?.selection?.vault === note.vault &&
      this.current.grant.account === note.account
    ) {
      this.#publish({
        grant: this.current.grant,
        notes: this.current.notes.map((entry) =>
          entry.path === saved.path ? saved : entry,
        ),
      });
    }
    return saved;
  }

  async apply(
    note: SavedNote,
    remote: DraftRemote,
  ): Promise<"saved" | "changed" | "missing"> {
    const draft = note.draft;
    if (!draft) return "saved";
    const found = await this.#admitted(
      note.account,
      note.vault,
      async (_grant, _settings, store) =>
        request<SavedNote | undefined>(
          store.get([note.account, note.vault, note.path]),
        ),
    );
    if (found?.draft?.revision !== draft.revision)
      throw Error("The local draft changed. Open it again before saving.");
    const version = await remote.read(note.path);
    if (!version) return "missing";
    if (version.text !== draft.text) {
      if (version.etag !== draft.baseEtag) return "changed";
      await remote.write(note.path, draft.text, draft.baseEtag);
    }
    await this.#admitted(
      note.account,
      note.vault,
      async (_grant, _settings, store) => {
        const key = [note.account, note.vault, note.path];
        const existing = await request<SavedNote | undefined>(store.get(key));
        if (existing?.draft?.revision === draft.revision) {
          const { draft: _savedDraft, ...snapshot } = existing;
          // The next admitted read refreshes the snapshot with a server ETag.
          store.put(snapshot);
        }
      },
    );
    await this.refresh();
    return "saved";
  }

  async #admitted<T>(
    account: string,
    vault: string,
    action: (
      grant: OfflineGrant,
      settings: IDBObjectStore,
      notes: IDBObjectStore,
    ) => Promise<T>,
  ): Promise<T | undefined> {
    if (this.#restricted) return;
    return this.database.transact("readwrite", async (settings, notes) => {
      const grant = await request<OfflineGrant | undefined>(
        settings.get(grantKey),
      );
      if (
        this.#restricted ||
        !validGrant(grant) ||
        grant.account !== account ||
        !grant.vaults.some((entry) => entry.id === vault)
      )
        return;
      return action(grant, settings, notes);
    });
  }

  #publish(snapshot: Snapshot): void {
    this.#snapshot = snapshot;
    this.#store.set(snapshot);
    clearTimeout(this.#timer);
    if (snapshot.grant) {
      const delay = Math.max(0, snapshot.grant.expiresAt - Date.now());
      this.#timer = setTimeout(
        () => {
          if (!validGrant(this.#snapshot.grant)) this.#publish({ notes: [] });
          else this.#publish(this.#snapshot);
        },
        Math.min(delay, maxDelay),
      );
    }
  }
}
