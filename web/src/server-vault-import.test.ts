import { strFromU8, unzipSync } from "fflate";
import { describe, expect, it, vi } from "vitest";

import type {
  BrowserFileSystemDirectoryHandle,
  BrowserFileSystemFileHandle,
} from "@lapis-notes/api";
import {
  archiveServerImportDirectory,
  canPickServerImportDirectory,
} from "./server-vault-import";

function fileHandle(
  name: string,
  content: string,
): BrowserFileSystemFileHandle {
  return {
    kind: "file",
    name,
    getFile: async () =>
      new Blob([content], { type: "text/markdown" }) as Blob & {
        name?: string;
      },
    createWritable: vi.fn(),
  };
}

function directoryHandle(
  name: string,
  entries: Array<
    [string, BrowserFileSystemFileHandle | BrowserFileSystemDirectoryHandle]
  >,
): BrowserFileSystemDirectoryHandle {
  return {
    kind: "directory",
    name,
    getFileHandle: vi.fn(),
    getDirectoryHandle: vi.fn(),
    removeEntry: vi.fn(),
    async *entries() {
      for (const entry of entries) yield entry;
    },
  };
}

describe("server vault directory import", () => {
  it("archives a picked folder with stable relative paths and progress", async () => {
    const progress = vi.fn();
    const root = directoryHandle("Project", [
      [
        "Notes",
        directoryHandle("Notes", [
          ["Welcome.md", fileHandle("Welcome.md", "# Welcome")],
        ]),
      ],
      ["README.md", fileHandle("README.md", "# Project")],
    ]);

    const archive = await archiveServerImportDirectory(root, {
      onProgress: progress,
    });
    const files = unzipSync(archive.bytes);

    expect(Object.keys(files)).toEqual(["Notes/Welcome.md", "README.md"]);
    expect(strFromU8(files["Notes/Welcome.md"]!)).toBe("# Welcome");
    expect(strFromU8(files["README.md"]!)).toBe("# Project");
    expect(archive.fileCount).toBe(2);
    expect(archive.sourceBytes).toBe(18);
    expect(progress).toHaveBeenLastCalledWith({
      collectedFiles: 2,
      collectedBytes: 18,
      currentPath: "README.md",
    });
  });

  it("rejects empty folders and unsafe mock entry names", async () => {
    await expect(
      archiveServerImportDirectory(directoryHandle("Empty", [])),
    ).rejects.toThrow("does not contain any files");
    await expect(
      archiveServerImportDirectory(
        directoryHandle("Unsafe", [["../note.md", fileHandle("note.md", "x")]]),
      ),
    ).rejects.toThrow("invalid entry name");
  });

  it("detects File System Access support without hiding ZIP fallback", () => {
    expect(
      canPickServerImportDirectory({ showDirectoryPicker: vi.fn() } as never),
    ).toBe(true);
    expect(canPickServerImportDirectory({} as never)).toBe(false);
  });
});
