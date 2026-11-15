import { mount } from "svelte";
import { getBootstrapAppearanceMode } from "@lapis-notes/api";
import "@lapismd/design-core/styles.css";
import "@lapismd/design-core/themes/lapis.css";
import "@lapis-notes/ui/theme.css";
import "@lapis-notes/ui/codemirror-autocomplete.css";
import "./styles/host.css";
import "./styles/login.css";
import "./styles/vault-launcher.css";
import "./styles/vault-dialogs.css";
import { initializeServerHostDocument } from "./server-host-document";
import { createRustHostedBackend } from "./hosted/rust/backend";
import ServerVaultHost from "./ServerVaultHost.svelte";
import SyncAccountConsent from "./SyncAccountConsent.svelte";

initializeServerHostDocument(await getBootstrapAppearanceMode());
const target = document.getElementById("app");
if (!target) throw new Error("Server host root is missing");

const connectionId = new URLSearchParams(window.location.search).get(
  "sync-connect",
);
export default connectionId !== null
  ? mount(SyncAccountConsent, {
      target,
      props: { requestId: connectionId },
    })
  : mount(ServerVaultHost, {
      target,
      props: {
        backend: createRustHostedBackend(undefined, {
          nativeMetadata: true,
          nativeHistory: true,
          nativeTasks: true,
        }),
      },
    });
