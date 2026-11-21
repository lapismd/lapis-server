import { mount, unmount } from "svelte";
import type { App } from "@lapis-notes/api";
import ConnectionStatus from "./ConnectionStatus.svelte";
import type { ComponentProps } from "svelte";

export function registerConnectionStatus(
  app: App,
  props: ComponentProps<typeof ConnectionStatus>,
): () => void {
  return app.statusBar.registerItem({
    id: "lapis-server:connection",
    alignment: "right",
    priority: Number.MAX_SAFE_INTEGER,
    mount(target) {
      const component = mount(ConnectionStatus, {
        target,
        props,
      });
      return () => {
        void unmount(component);
      };
    },
  });
}
