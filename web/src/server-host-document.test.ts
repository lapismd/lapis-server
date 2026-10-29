import { describe, expect, it } from "vitest";

import { initializeServerHostDocument } from "./server-host-document";

describe("initializeServerHostDocument", () => {
  it("publishes the explicit online runtime", () => {
    initializeServerHostDocument();

    expect(document.documentElement.dataset.runtime).toBe("rust-server");
    expect(document.documentElement.dataset.pwaHost).toBeUndefined();
  });
});
