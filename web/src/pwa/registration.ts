/// <reference types="vite-plugin-pwa/client" />
import { writable } from "svelte/store";
import { registerSW } from "virtual:pwa-register";

type InstallPrompt = Event & { prompt(): Promise<unknown> };
type PwaState = {
  canInstall: boolean;
  updateAvailable: boolean;
  offlineReady: boolean;
};

export function registerServerPwa() {
  const state = writable<PwaState>({
    canInstall: false,
    updateAvailable: false,
    offlineReady: false,
  });
  let prompt: InstallPrompt | undefined;
  let update: ((reload?: boolean) => Promise<void>) | undefined;
  const change = (value: Partial<PwaState>) =>
    state.update((old) => ({ ...old, ...value }));
  if (
    import.meta.env.PROD &&
    window.isSecureContext &&
    "serviceWorker" in navigator
  ) {
    const capture = (event: Event) => {
      event.preventDefault();
      prompt = event as InstallPrompt;
      change({ canInstall: true });
    };
    const installed = () => {
      prompt = undefined;
      change({ canInstall: false });
    };
    const check = () => {
      if (document.visibilityState !== "visible") return;
      void navigator.serviceWorker
        .getRegistration()
        .then((registration) => registration?.update())
        .catch(() => {});
    };
    window.addEventListener("beforeinstallprompt", capture);
    window.addEventListener("appinstalled", installed);
    window.addEventListener("online", check);
    document.addEventListener("visibilitychange", check);
    window.addEventListener("pagehide", (event) => {
      if (event.persisted) return;
      window.removeEventListener("beforeinstallprompt", capture);
      window.removeEventListener("appinstalled", installed);
      window.removeEventListener("online", check);
      document.removeEventListener("visibilitychange", check);
    });
    update = registerSW({
      immediate: true,
      onNeedRefresh: () => change({ updateAvailable: true }),
      onOfflineReady: () => change({ offlineReady: true }),
      onRegisteredSW: (_url, registration) => {
        if (registration?.active) change({ offlineReady: true });
      },
    });
  }
  return {
    subscribe: state.subscribe,
    async install() {
      await prompt?.prompt();
    },
    async applyUpdate(persist: () => Promise<void>) {
      if (!update) return;
      await persist();
      await update(true);
    },
  };
}

export type ServerPwa = ReturnType<typeof registerServerPwa>;
