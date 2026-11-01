import type { BootstrapAppearanceMode } from "@lapis-notes/api";

export function initializeServerHostDocument(
  mode: BootstrapAppearanceMode = "system",
): void {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.runtime = "rust-server";
  document.documentElement.dataset.uiTheme = "lapis";
  const dark =
    mode === "dark" ||
    (mode === "system" &&
      globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches);
  for (const name of ["dark", "theme-dark"])
    document.documentElement.classList.toggle(name, !!dark);
  for (const name of ["light", "theme-light"])
    document.documentElement.classList.toggle(name, !dark);
}
