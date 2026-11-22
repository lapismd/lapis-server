import {
  AppBridge,
  PostMessageTransport,
} from "@modelcontextprotocol/ext-apps/app-bridge";
import type { CallToolResult } from "@modelcontextprotocol/client";

declare function relayTool(
  name: string,
  args: Record<string, unknown>,
): Promise<CallToolResult>;

window.addEventListener("message", ({ source, origin, data }) => {
  if (
    source !== window ||
    origin !== location.origin ||
    data?.type !== "open-app"
  )
    return;
  const frame = document.createElement("iframe");
  frame.title = "External sandbox";
  frame.sandbox.add("allow-scripts", "allow-same-origin", "allow-forms");
  frame.src = "http://127.0.0.1:58192/sandbox";
  frame.style.cssText = "border:0;width:100%;height:720px";
  document.body.replaceChildren(frame);
  const transport = new PostMessageTransport(
    frame.contentWindow!,
    frame.contentWindow!,
  );
  const bridge = new AppBridge(
    null,
    { name: "External acceptance host", version: "1.0.0" },
    { serverTools: {} },
  );
  bridge.onsandboxready = () => {
    void bridge.sendSandboxResourceReady({
      html: data.html,
      sandbox: "allow-scripts allow-forms",
    });
  };
  bridge.oninitialized = () => {
    void (async () => {
      await bridge.sendToolInput({ arguments: data.args });
      await bridge.sendToolResult(data.result);
    })();
  };
  bridge.oncalltool = ({ name, arguments: args }) =>
    relayTool(name, args ?? {});
  void bridge.connect(transport);
});
