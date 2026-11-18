import { isMap, parseDocument } from "yaml";
import { isDeepStrictEqual } from "node:util";
export function verifyWorkspaceNotes(paths, files, expected) {
  for (const path of paths) {
    const text = files[path];
    if (
      typeof text !== "string" ||
      !text.isWellFormed() ||
      Buffer.byteLength(text) > 8192
    )
      throw Error("Invalid managed dependency note bytes");
    const lines = text.split(/\r?\n/u),
      end = lines.indexOf("---", 1);
    if (
      lines[0] !== "---" ||
      end < 1 ||
      !lines
        .slice(end + 1)
        .join("\n")
        .trim()
    )
      throw Error("Invalid managed dependency note body");
    const header = lines.slice(1, end).join("\n"),
      document = parseDocument(header);
    if (
      document.errors.length ||
      document.warnings.length ||
      (header.trim() && !isMap(document.contents))
    )
      throw Error("Invalid managed dependency note release map");
    const release = header.trim() ? document.toJSON() : {};
    if (!isDeepStrictEqual(release, expected))
      throw Error("Unexpected managed dependency release intent");
  }
  if (Object.keys(expected).length && !paths.length)
    throw Error("Runtime dependency patch requires its Changeset");
}
