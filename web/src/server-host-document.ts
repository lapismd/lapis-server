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

export function observeServerFrameColor(
  mode: BootstrapAppearanceMode,
): () => void {
  const root = document.documentElement;
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const meta = document.createElement("meta");
  meta.name = "theme-color";
  meta.dataset.lapisHostThemeColor = "true";
  for (const fallback of document.querySelectorAll(
    "[data-lapis-bootstrap-theme-color]",
  ))
    fallback.remove();
  document.head.append(meta);
  const update = () => {
    const background = getComputedStyle(root)
      .getPropertyValue("--ui-workspace-tab-container-background")
      .trim();
    meta.content = CSS.supports("color", background)
      ? background
      : root.classList.contains("dark")
        ? "#1f1f1f"
        : "#f6f6f6";
  };
  const onMedia = () => {
    if (
      mode === "system" &&
      !document.querySelector("[data-ui-component=lapis-workspace-shell]")
    )
      initializeServerHostDocument("system");
    update();
  };
  const observer = new MutationObserver(update);
  observer.observe(root, { attributeFilter: ["class", "data-ui-theme"] });
  media.addEventListener("change", onMedia);
  update();
  return () => {
    observer.disconnect();
    media.removeEventListener("change", onMedia);
    meta.remove();
  };
}
