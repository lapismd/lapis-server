import { expect } from "storybook/test";

export function expectShellFillsViewport(canvasElement: HTMLElement) {
  const storyRoot =
    (canvasElement.closest("#storybook-root") as HTMLElement | null) ??
    canvasElement;
  const viewport = canvasElement.ownerDocument.documentElement;
  const shell = canvasElement.querySelector(
    '[data-ui-component="lapis-workspace-shell"]',
  );
  expect(shell).toBeTruthy();
  const rootBox = shell!.getBoundingClientRect();
  expect(
    canvasElement.ownerDocument.body.hasAttribute(
      "data-ui-catalog-full-viewport",
    ),
  ).toBe(true);

  expect(getComputedStyle(storyRoot).padding).toBe("0px");
  expect(Math.abs(rootBox.height - viewport.clientHeight)).toBeLessThan(1);
  expect(Math.abs(rootBox.width - viewport.clientWidth)).toBeLessThan(1);
}
