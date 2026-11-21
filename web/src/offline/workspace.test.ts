import { IDBFactory } from "fake-indexeddb";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OfflineDatabase } from "./database";
import { OfflineWorkspace, isOfflineMarkdown } from "./workspace";

const account = "owner";
const vault = "notes";
const path = "Note.md";
const version = { text: "# Server note", etag: '"' + "a".repeat(64) + '"' };
let database: OfflineDatabase;
let offline: OfflineWorkspace;
let stop: () => void;

beforeEach(async () => {
  const browser = new IDBFactory();
  database = new OfflineDatabase(() => browser.open("offline-test", 1));
  offline = new OfflineWorkspace(database);
  stop = offline.start();
  await offline.enable(
    account,
    Date.now() + 60_000,
    [{ id: vault, name: "Notes" }],
    vault,
  );
  await offline.cache(account, vault, path, version);
  await offline.select(account, vault, path);
});

afterEach(() => stop());

describe("trusted offline drafts", () => {
  it("restores the selected note and latest draft in a new workspace", async () => {
    const note = offline.current.notes[0];
    await offline.stage(note, "# Local draft");
    const reopened = new OfflineWorkspace(database);
    const close = reopened.start();
    try {
      await reopened.refresh();
      expect(reopened.current.grant?.selection).toEqual({ vault, path });
      expect(reopened.current.notes[0].draft?.text).toBe("# Local draft");
    } finally {
      close();
    }
  });

  it("keeps local work after a conflict and uses the original strong ETag", async () => {
    const saved = await offline.stage(
      offline.current.notes[0],
      "# Local draft",
    );
    const other = { text: "# Other version", etag: '"different"' };
    const remote = { read: vi.fn().mockResolvedValue(other), write: vi.fn() };
    expect(await offline.apply(saved, remote)).toBe("changed");
    expect(remote.write).not.toHaveBeenCalled();
    expect(offline.current.notes[0].draft?.text).toBe("# Local draft");
    remote.read.mockResolvedValue(version);
    await expect(offline.apply(saved, remote)).resolves.toBe("saved");
    expect(remote.write).toHaveBeenCalledExactlyOnceWith(
      path,
      "# Local draft",
      version.etag,
    );
    expect(offline.current.notes[0].draft).toBeUndefined();
  });

  it("checks an uncertain write before any subsequent save attempt", async () => {
    const text = "# Local draft";
    const saved = await offline.stage(offline.current.notes[0], text);
    const remote = {
      read: vi.fn().mockResolvedValue(version),
      write: vi.fn().mockRejectedValue(Error("connection lost")),
    };
    await expect(offline.apply(saved, remote)).rejects.toThrow(
      "connection lost",
    );
    remote.read.mockResolvedValue({ ...version, text });
    await expect(offline.apply(saved, remote)).resolves.toBe("saved");
    expect(remote.write).toHaveBeenCalledTimes(1);
  });

  it("rejects stale tabs and revokes data on account or vault changes", async () => {
    const old = offline.current.notes[0];
    await offline.stage(old, "First tab");
    await expect(offline.stage(old, "Second tab")).rejects.toThrow(
      "another tab",
    );
    await offline.authorize(account, Date.now() + 60_000, [
      { id: "elsewhere", name: "Other vault" },
    ]);
    expect(offline.current.notes).toEqual([]);
    expect(await offline.getNote(account, vault, path)).toBeUndefined();
    await offline.authorize("different account", Date.now() + 60_000, []);
    expect(offline.current.grant).toBeUndefined();
  });

  it("clears local copies and stops sessions using the old grant", async () => {
    const old = offline.current.notes[0];
    await offline.clear();
    expect(offline.current.notes).toEqual([]);
    await expect(offline.stage(old, "Stale edit")).rejects.toThrow("locked");
    await offline.refresh();
    expect(offline.current.grant).toBeUndefined();
  });

  it.each([
    ".lapis/config.md",
    "../Note.md",
    "nested/../Note.md",
    "secret.txt",
  ])("excludes %s from the note cache", (path) => {
    expect(isOfflineMarkdown(path)).toBe(false);
  });
});
