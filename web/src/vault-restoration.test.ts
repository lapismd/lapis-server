import { describe, expect, it } from "vitest";
import { createVaultRestoration } from "./vault-restoration";

function fixture() {
  const records = new Map<string, string>();
  const storage = {
    getItem: (key: string) => records.get(key) ?? null,
    setItem: (key: string, value: string) => {
      records.set(key, value);
    },
    removeItem: (key: string) => {
      records.delete(key);
    },
  };
  return {
    records,
    storage,
    restoration: createVaultRestoration(() => storage),
  };
}

describe("owner-scoped vault restoration", () => {
  it("stores only a versioned vault ID and consumes the attempt before boot", () => {
    const { records, restoration } = fixture();
    restoration.remember("owner", "notes");
    expect([...records.values()].map((value) => JSON.parse(value))).toEqual([
      { schemaVersion: 1, vaultId: "notes" },
    ]);
    expect(restoration.take("owner")).toBe("notes");
    expect(restoration.take("owner")).toBeNull();
    expect(records.size).toBe(0);
    restoration.remember("owner", "notes");
    expect(restoration.take("owner")).toBe("notes");
  });

  it("does not restore or erase another authenticated owner's choice", () => {
    const { restoration } = fixture();
    restoration.remember("first owner", "first-vault");
    restoration.remember("second owner", "second-vault");
    restoration.forget("first owner");
    expect(restoration.take("first owner")).toBeNull();
    expect(restoration.take("second owner")).toBe("second-vault");
  });

  it.each([
    "{",
    "null",
    JSON.stringify({ schemaVersion: 2, vaultId: "notes" }),
    JSON.stringify({ schemaVersion: 1, vaultId: 12 }),
    JSON.stringify({ schemaVersion: 1, vaultId: "" }),
    JSON.stringify({ schemaVersion: 1, vaultId: "x".repeat(513) }),
    "x".repeat(1025),
  ])("retires malformed or oversized state without retrying: %s", (value) => {
    const { records, restoration } = fixture();
    restoration.remember("owner", "notes");
    const key = [...records.keys()][0];
    records.set(key, value);
    expect(restoration.take("owner")).toBeNull();
    expect(records.has(key)).toBe(false);
  });

  it("does not require browser storage to open or leave a workspace", () => {
    const restoration = createVaultRestoration(() => {
      throw new DOMException("Storage is blocked", "SecurityError");
    });
    expect(restoration.take("owner")).toBeNull();
    expect(() => restoration.remember("owner", "notes")).not.toThrow();
    expect(() => restoration.forget("owner")).not.toThrow();
  });

  it("does not restore an intent it cannot retire", () => {
    const { storage } = fixture();
    const restoration = createVaultRestoration(() => ({
      ...storage,
      removeItem: () => {
        throw new Error("Read-only storage");
      },
    }));
    restoration.remember("owner", "notes");
    expect(restoration.take("owner")).toBeNull();
  });
});
