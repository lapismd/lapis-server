import type { BootstrapAppearanceMode } from "@lapis-notes/api";
import {
  applyWebHostAppearance,
  initializePwaHostDocument,
  observeHostThemeColor,
  observeWebHostAppearance,
  registerPwaWindowControlsOverlay,
} from "@lapis-notes/app-profile/browser-presentation";

export function initializeServerHostDocument(
  mode: BootstrapAppearanceMode = "system",
): void {
  initializePwaHostDocument("rust-server");
  applyWebHostAppearance(mode);
}

export function observeServerFrameColor(
  mode: BootstrapAppearanceMode,
): () => void {
  const stopAppearance = observeWebHostAppearance(mode);
  const stopThemeColor = observeHostThemeColor();
  const stopWindowControls = registerPwaWindowControlsOverlay();
  return () => {
    stopWindowControls();
    stopThemeColor();
    stopAppearance();
  };
}
