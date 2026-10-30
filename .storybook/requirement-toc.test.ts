import { expect, it } from "vitest";
import { requirementTocLabel } from "./requirement-toc";

it("labels a requirement heading with its id", () => {
  expect(requirementTocLabel("SRV-SRV-001 — Public image")).toBe("SRV-SRV-001");
  expect(requirementTocLabel("DC-SHELL-001 — Shared Shell invariants")).toBe(
    "DC-SHELL-001",
  );
});

it("leaves a heading that is not a requirement id unchanged", () => {
  expect(requirementTocLabel("Acceptance details")).toBe("Acceptance details");
});
