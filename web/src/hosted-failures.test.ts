import { describe, expect, it } from "vitest";
import { withHostedCleanup } from "../e2e/hosted-failures";

describe("hosted scenario failure ownership", () => {
  it("keeps primary and every cleanup failure in encounter order", async () => {
    const primary = new Error("native reference could not restore");
    const first = new Error("guarded logout refused");
    const last = new Error("runtime error retained during shutdown");
    const calls: string[] = [];
    const run = withHostedCleanup(async () => {
      throw primary;
    }, [
      async () => {
        calls.push("logout");
        throw first;
      },
      async () => {
        calls.push("context close");
      },
      async () => {
        calls.push("host stop");
        throw last;
      },
    ]);
    await expect(run).rejects.toMatchObject({
      errors: [primary, first, last],
      cause: primary,
    });
    expect(calls).toEqual(["logout", "context close", "host stop"]);
  });

  it("preserves the original error when its diagnostic attachment also fails", async () => {
    const primary = new Error("original browser failure");
    const diagnostic = new Error("attachment failed");
    let cleaned = false;
    await expect(
      withHostedCleanup(
        async () => {
          throw primary;
        },
        [
          async () => {
            cleaned = true;
          },
        ],
        async (error) => {
          expect(error).toBe(primary);
          throw diagnostic;
        },
      ),
    ).rejects.toMatchObject({ errors: [primary, diagnostic], cause: primary });
    expect(cleaned).toBe(true);
  });

  it("rethrows a lone scenario failure unchanged", async () => {
    const primary = new Error("exact regression");
    await expect(
      withHostedCleanup(async () => {
        throw primary;
      }, [async () => {}]),
    ).rejects.toBe(primary);
  });

  it("fails for cleanup alone instead of reporting a passing scenario", async () => {
    const cleanup = new Error("host did not exit cleanly");
    await expect(
      withHostedCleanup(async () => {}, [
        async () => {
          throw cleanup;
        },
      ]),
    ).rejects.toBe(cleanup);
  });

  it("preserves every cleanup failure without a scenario failure", async () => {
    const first = new Error("context shutdown");
    const second = new Error("service shutdown");
    await expect(
      withHostedCleanup(async () => {}, [
        async () => {
          throw first;
        },
        async () => {
          throw second;
        },
      ]),
    ).rejects.toMatchObject({ errors: [first, second], cause: first });
  });

  it("runs diagnostics before cleanup and accepts a successful complete drain", async () => {
    const calls: string[] = [];
    await withHostedCleanup(
      async () => {
        calls.push("scenario");
      },
      [
        async () => {
          calls.push("first release");
        },
        async () => {
          calls.push("last release");
        },
      ],
      async () => {
        calls.push("unexpected diagnostic");
      },
    );
    expect(calls).toEqual(["scenario", "first release", "last release"]);
  });
});
