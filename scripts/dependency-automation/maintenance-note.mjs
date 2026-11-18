import { isDeepStrictEqual } from "node:util";
import { parse } from "yaml";
export const managedMaintenanceNote = (path) =>
  /^\.changeset\/lapis-dependencies-[a-f0-9]{12}\.md$/.test(path);
export function verifyMaintenanceScope(changedFiles, files, allowedPaths) {
  const notes = changedFiles.filter(managedMaintenanceNote);
  if (
    notes.length > 1 ||
    changedFiles.some(
      (path) => !allowedPaths.includes(path) && !notes.includes(path),
    )
  )
    throw Error("Unexpected development intake source scope");
  for (const path of notes) {
    const bytes = files[path];
    if (
      typeof bytes !== "string" ||
      !bytes.isWellFormed() ||
      Buffer.byteLength(bytes) > 8192
    )
      throw Error("Development intake requires bounded managed note bytes");
    const front =
      typeof bytes === "string" &&
      /^---\r?\n([\s\S]*?)^---[ \t]*(?:\r?\n|$)/m.exec(bytes);
    if (
      !front ||
      front.index !== 0 ||
      !isDeepStrictEqual(front[1].trim() ? parse(front[1]) : {}, {})
    )
      throw Error(
        "Development intake requires one empty managed maintenance note",
      );
  }
}
