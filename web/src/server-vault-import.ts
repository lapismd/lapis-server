import type {
  BrowserFileSystemDirectoryHandle,
  BrowserFileSystemFileHandle,
} from "@lapis-notes/api";
import { zipSync } from "fflate";

const MAX_ARCHIVE_BYTES = 512 * 1024 * 1024;
const MAX_EXPANDED_BYTES = 2 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES = 100_000;

export interface ServerDirectoryImportProgress {
  collectedFiles: number;
  collectedBytes: number;
  currentPath: string | null;
}

export interface ServerDirectoryArchive {
  bytes: Uint8Array;
  fileCount: number;
  sourceBytes: number;
}

export interface ServerDirectoryArchiveOptions {
  onProgress?: (
    progress: ServerDirectoryImportProgress,
  ) => void | Promise<void>;
}

interface DirectoryPickerHost {
  showDirectoryPicker?: unknown;
}

function assertSafeEntryName(name: string): string {
  if (
    !name ||
    name === "." ||
    name === ".." ||
    name.includes("/") ||
    name.includes("\\") ||
    name.includes("\0")
  ) {
    throw new Error(
      `The selected folder contains an invalid entry name: ${name}`,
    );
  }
  return name;
}

async function directoryEntries(
  directory: BrowserFileSystemDirectoryHandle,
): Promise<
  Array<
    [string, BrowserFileSystemFileHandle | BrowserFileSystemDirectoryHandle]
  >
> {
  const iterator = directory.entries?.() ?? directory[Symbol.asyncIterator]?.();
  if (!iterator) {
    throw new Error("This browser cannot enumerate the selected folder");
  }
  const entries: Array<
    [string, BrowserFileSystemFileHandle | BrowserFileSystemDirectoryHandle]
  > = [];
  for await (const [name, handle] of iterator) {
    entries.push([assertSafeEntryName(name), handle]);
  }
  entries.sort(([left], [right]) => left.localeCompare(right));
  return entries;
}

export function canPickServerImportDirectory(
  host: DirectoryPickerHost = globalThis as DirectoryPickerHost,
): boolean {
  return typeof host.showDirectoryPicker === "function";
}

export async function archiveServerImportDirectory(
  root: BrowserFileSystemDirectoryHandle,
  options: ServerDirectoryArchiveOptions = {},
): Promise<ServerDirectoryArchive> {
  const files = Object.create(null) as Record<string, Uint8Array>;
  let fileCount = 0;
  let sourceBytes = 0;

  async function collect(
    directory: BrowserFileSystemDirectoryHandle,
    parentPath = "",
  ): Promise<void> {
    for (const [name, handle] of await directoryEntries(directory)) {
      const path = parentPath ? `${parentPath}/${name}` : name;
      if (handle.kind === "directory") {
        await collect(handle, path);
        continue;
      }
      fileCount += 1;
      if (fileCount > MAX_ARCHIVE_ENTRIES) {
        throw new Error("The selected folder contains too many files");
      }
      const file = await handle.getFile();
      const bytes = new Uint8Array(await file.arrayBuffer());
      sourceBytes += bytes.byteLength;
      if (sourceBytes > MAX_EXPANDED_BYTES) {
        throw new Error("The selected folder is too large to import");
      }
      files[path] = bytes;
      await options.onProgress?.({
        collectedFiles: fileCount,
        collectedBytes: sourceBytes,
        currentPath: path,
      });
    }
  }

  await collect(root);
  if (!fileCount) {
    throw new Error("The selected folder does not contain any files");
  }
  const bytes = zipSync(files, { level: 6 });
  if (bytes.byteLength > MAX_ARCHIVE_BYTES) {
    throw new Error("The selected folder is too large to upload");
  }
  return { bytes, fileCount, sourceBytes };
}

export const serverDirectoryImportLimits = {
  archiveBytes: MAX_ARCHIVE_BYTES,
  expandedBytes: MAX_EXPANDED_BYTES,
  entries: MAX_ARCHIVE_ENTRIES,
} as const;
