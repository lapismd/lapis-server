import { expect } from "storybook/test";

export function expectShellFillsViewport(canvasElement: HTMLElement) {
  const storyRoot =
    (canvasElement.closest("#storybook-root") as HTMLElement | null) ??
    canvasElement;
  const viewport = canvasElement.ownerDocument.documentElement;
  const rootBox = storyRoot.getBoundingClientRect();
  expect(
    canvasElement.ownerDocument.body.hasAttribute(
      "data-ui-catalog-full-viewport",
    ),
  ).toBe(true);
  expect(viewport.getAttribute("data-ui-theme")).toBe("lapis");
  expect(getComputedStyle(storyRoot).padding).toBe("0px");
  expect(Math.abs(rootBox.height - viewport.clientHeight)).toBeLessThan(1);
  expect(Math.abs(rootBox.width - viewport.clientWidth)).toBeLessThan(1);
}
