import { describe, expect, it, vi } from "vitest";
import { createRustSkillManagement } from "./skill-management";
import type { RustVaultClient } from "./vault-client";
describe("native hosted skill management", () => {
  it("preserves admission identity and returns skipped local edits", async () => {
    const json = vi.fn().mockResolvedValue({
      outcome: {
        status: "ok",
        value: [
          {
            name: "lapis-markdown",
            status: "skipped",
            reason: "locally-modified",
          },
        ],
      },
    });
    const requireOpen = vi.fn();
    const client = {
      path: "/clients/admitted",
      host: { json },
      requireOpen,
    } as unknown as RustVaultClient;
    const result = await createRustSkillManagement(client).manage(
      "update",
      "lapis-markdown",
    );
    expect(json).toHaveBeenCalledWith(
      "/clients/admitted/plugins/skills/operations/update",
      "POST",
      expect.objectContaining({
        version: 1,
        input: { skill: "lapis-markdown" },
      }),
    );
    expect(result[0].reason).toBe("locally-modified");
    expect(requireOpen).toHaveBeenCalledOnce();
    requireOpen.mockImplementation(() => {
      throw Error("detached");
    });
    await expect(
      createRustSkillManagement(client).manage("install"),
    ).rejects.toThrow("detached");
    expect(json).toHaveBeenCalledOnce();
  });
});
