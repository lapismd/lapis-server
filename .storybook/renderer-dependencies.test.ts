import { realpathSync } from "node:fs";
import { createRequire, findPackageJSON } from "node:module";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";
import { rendererInclusions } from "../web/renderer-dependencies";

// Intermediate owners may expose a Svelte-only root. Locate their public npm
// metadata, then resolve the final public prebundle entry without executing it.
describe.each(rendererInclusions)("renderer optimizer input %s", (entry) => {
  it("resolves against the frozen registry graph", () => {
    let resolve = createRequire(import.meta.url);
    let base = import.meta.url;
    const parts = entry.split(" > ");
    for (const owner of parts.slice(0, -1)) {
      const manifest = findPackageJSON(owner, base);
      expect(manifest, owner).toBeDefined();
      const file = realpathSync(manifest!);
      resolve = createRequire(file);
      base = pathToFileURL(file).href;
    }
    const final = parts.at(-1)!;
    const publicEntry = final.endsWith("/**") ? final.slice(0, -3) : final;
    expect(resolve.resolve(publicEntry)).toBeTruthy();
  });
});
