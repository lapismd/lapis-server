export function initializeServerHostDocument(): void {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.runtime = "rust-server";
}
