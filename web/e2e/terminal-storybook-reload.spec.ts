import { expect, test, type FrameLocator } from "@playwright/test";

async function expectTerminal(frame: FrameLocator) {
  await expect(
    frame.getByRole("button", { name: "Open left sidebar", exact: true }),
  ).toBeVisible();
  const surface = frame.getByTestId("terminal-surface");
  await expect(surface.locator("canvas")).toBeVisible();
  await expect
    .poll(() =>
      surface.evaluate((element) => {
        const bitmap = element.querySelector("canvas")!;
        return Math.abs(
          bitmap.getBoundingClientRect().right -
            element.getBoundingClientRect().right,
        );
      }),
    )
    .toBeLessThan(1);
  await expect
    .poll(() =>
      surface.evaluate((element) => {
        const canvas = element.querySelector("canvas")!;
        const reference = document.createElement("canvas").getContext("2d")!;
        reference.fillStyle = getComputedStyle(element).backgroundColor;
        reference.fillRect(0, 0, 1, 1);
        const expected = [...reference.getImageData(0, 0, 1, 1).data].join(",");
        const pixels = canvas
          .getContext("2d")!
          .getImageData(0, 0, canvas.width, canvas.height).data;
        const counts = new Map<string, number>();
        for (let index = 0; index < pixels.length; index += 4) {
          const key = [...pixels.subarray(index, index + 4)].join(",");
          counts.set(key, (counts.get(key) ?? 0) + 1);
        }
        return [...counts].sort((a, b) => b[1] - a[1])[0][0] === expected;
      }),
    )
    .toBe(true);
}

for (const mode of ["light", "dark"] as const) {
  test(`${mode} Server terminal survives Storybook reload without corrupt glyphs`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(() => {
      const state = window as Window & {
        terminalReviewGlyphs?: WeakMap<
          HTMLCanvasElement,
          { count: number; invalid: string[]; text: string }
        >;
      };
      state.terminalReviewGlyphs = new WeakMap();
      const original = CanvasRenderingContext2D.prototype.fillText;
      CanvasRenderingContext2D.prototype.fillText = function (
        ...args: Parameters<typeof original>
      ) {
        if (this.canvas.closest('[data-testid="terminal-surface"]')) {
          const capture = state.terminalReviewGlyphs!.get(this.canvas) ?? {
            count: 0,
            invalid: [],
            text: "",
          };
          state.terminalReviewGlyphs!.set(this.canvas, capture);
          capture.count++;
          capture.text = (capture.text + args[0]).slice(-16384);
          if (!/^[\x20-\x7e·❯]*$/u.test(args[0]) && capture.invalid.length < 10)
            capture.invalid.push(args[0]);
        }
        return original.apply(this, args);
      };
    });
    const story = `shell-server-terminal--failed-command${mode === "dark" ? "-dark" : ""}`;
    await page.goto(`http://127.0.0.1:6091/?path=/story/${story}`);
    const frame = page.frameLocator("#storybook-preview-iframe");
    for (let reload = 0; reload < 5; reload++) {
      await expectTerminal(frame);
      const glyphs = await frame.locator("body").evaluate(
        () =>
          (
            window as Window & {
              terminalReviewGlyphs?: WeakMap<
                HTMLCanvasElement,
                { count: number; invalid: string[]; text: string }
              >;
            }
          ).terminalReviewGlyphs?.get(
            document.querySelector<HTMLCanvasElement>(
              '[data-testid="terminal-surface"] canvas',
            )!,
          ) ?? { count: 0, invalid: [], text: "" },
      );
      expect(glyphs.count).toBeGreaterThan(20);
      expect(glyphs.invalid).toEqual([]);
      const input = frame.locator('[data-testid="terminal-surface"] textarea');
      await input.pressSequentially("pwd");
      await input.press("Enter");
      await expect
        .poll(() =>
          frame.locator("body").evaluate(() => {
            const state = window as Window & {
              terminalReviewGlyphs?: WeakMap<
                HTMLCanvasElement,
                { text: string }
              >;
            };
            return (
              state.terminalReviewGlyphs?.get(
                document.querySelector<HTMLCanvasElement>(
                  '[data-testid="terminal-surface"] canvas',
                )!,
              )?.text ?? ""
            );
          }),
        )
        .toContain("92b3bbf0-07f6-468e-a424-06d7bbdba797");

      if (reload < 4) {
        const canvas = await frame
          .locator('[data-testid="terminal-surface"] canvas')
          .elementHandle();
        await page
          .getByRole("button", { name: "Reload story", exact: true })
          .click();
        await expect
          .poll(() =>
            canvas!.evaluate((node) => node.isConnected).catch(() => false),
          )
          .toBe(false);
      }
    }
    expect(errors).toEqual([]);
  });
}
