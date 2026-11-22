import { describe, expect, it } from "vitest";

import { initializeServerHostDocument } from "./server-host-document";

describe("initializeServerHostDocument", () => {
  it("publishes the explicit online runtime", () => {
    initializeServerHostDocument();

    expect(document.documentElement.dataset.uiTheme).toBe("lapis");
    expect(document.documentElement.dataset.runtime).toBe("rust-server");
    expect(document.documentElement.dataset.pwaHost).toBe("true");
    expect(document.documentElement.dataset.pwaTitlebarHidden).toBe("false");
    expect(document.documentElement.dataset.engine).toBeTruthy();
    expect(document.documentElement.dataset.os).toBeTruthy();
  });
  it("applies both Design Core and retained editor appearance classes", () => {
    initializeServerHostDocument("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.classList.contains("theme-dark")).toBe(
      true,
    );
    initializeServerHostDocument("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(document.documentElement.classList.contains("theme-light")).toBe(
      true,
    );
  });
});
