export type OfflineVault = { id: string; name: string };

export type Draft = { text: string; baseEtag: string; revision: string };
export type SavedNote = {
  account: string;
  vault: string;
  path: string;
  text: string;
  etag: string;
  opened: number;
  draft?: Draft;
};
export type OfflineGrant = {
  account: string;
  expiresAt: number;
  vaults: OfflineVault[];
  selection?: { vault: string; path?: string };
};

export function request<T>(operation: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    operation.onsuccess = () => resolve(operation.result);
    operation.onerror = () => reject(operation.error);
  });
}

export class OfflineDatabase {
  #database?: Promise<IDBDatabase>;

  constructor(
    private readonly open = () => indexedDB.open("lapis-server-offline", 1),
  ) {}

  async transact<T>(
    mode: IDBTransactionMode,
    operation: (settings: IDBObjectStore, notes: IDBObjectStore) => Promise<T>,
  ): Promise<T> {
    const database = await (this.#database ??= this.#connect());
    const transaction = database.transaction(["settings", "notes"], mode);
    const complete = new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onabort = () =>
        reject(
          transaction.error ?? Error("Offline storage transaction aborted"),
        );
    });
    try {
      const [result] = await Promise.all([
        operation(
          transaction.objectStore("settings"),
          transaction.objectStore("notes"),
        ),
        complete,
      ]);
      return result;
    } catch (error) {
      try {
        transaction.abort();
      } catch {
        /* Already committed or aborted. */
      }
      throw error;
    }
  }

  async #connect(): Promise<IDBDatabase> {
    try {
      const open = this.open();
      open.onupgradeneeded = () => {
        open.result.createObjectStore("settings");
        const notes = open.result.createObjectStore("notes", {
          keyPath: ["account", "vault", "path"],
        });
        notes.createIndex("vault", ["account", "vault"]);
      };
      const database = await request(open);
      database.onversionchange = () => {
        database.close();
        this.#database = undefined;
      };
      return database;
    } catch (error) {
      // A new request is useful after the browser restores storage access.
      this.#database = undefined;
      throw error;
    }
  }
}
