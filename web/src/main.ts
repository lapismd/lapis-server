import { mount } from "svelte";
import "@lapismd/design-core/styles.css";
import "@lapismd/design-core/themes/lapis.css";
import { initializeServerHostDocument } from "./server-host-document";
import { createRustHostedBackend } from "./hosted/rust/backend";
import ServerVaultHost from "./ServerVaultHost.svelte";

initializeServerHostDocument();
const target = document.getElementById("app");
if (!target) throw new Error("Server host root is missing");

export default mount(ServerVaultHost, {
  target,
  props: { backend: createRustHostedBackend() },
});
