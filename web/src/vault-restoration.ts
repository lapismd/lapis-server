type IntentStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export interface VaultRestoration {
  take(owner: string): string | null;
  remember(owner: string, vaultId: string): void;
  forget(owner: string): void;
}

const keyFor = (owner: string) =>
  `lapis.server.workspace.v1:${encodeURIComponent(owner)}`;

/** Origin-scoped browser preference; never an admission or authentication grant. */
export function createVaultRestoration(
  storage: () => IntentStorage | undefined = () => globalThis.localStorage,
): VaultRestoration {
  return {
    take(owner) {
      if (!owner) return null;
      try {
        const store = storage();
        if (!store) return null;
        const value = store.getItem(keyFor(owner));
        if (!value) return null;
        // A failed boot must not repeat automatically on the next reload.
        store.removeItem(keyFor(owner));
        if (value.length > 1024) return null;
        const intent = JSON.parse(value);
        return intent?.schemaVersion === 1 &&
          typeof intent.vaultId === "string" &&
          intent.vaultId.length > 0 &&
          intent.vaultId.length <= 512
          ? intent.vaultId
          : null;
      } catch {
        return null;
      }
    },
    remember(owner, vaultId) {
      if (!owner || !vaultId) return;
      try {
        storage()?.setItem(
          keyFor(owner),
          JSON.stringify({ schemaVersion: 1, vaultId }),
        );
      } catch {
        // Private/blocked storage must not prevent an admitted workspace opening.
      }
    },
    forget(owner) {
      if (!owner) return;
      try {
        storage()?.removeItem(keyFor(owner));
      } catch {
        // The preference is optional; authentication remains server-owned.
      }
    },
  };
}
