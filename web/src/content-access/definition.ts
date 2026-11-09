import type { PluginSettingsDefinition } from "@lapis-notes/api";
import type { ContentAccessModel } from "./model";

export function definition(
  model: ContentAccessModel,
): PluginSettingsDefinition {
  const id = (name: string) => `content-access.${name}`;
  return {
    source: model,
    section: {
      id: "content-access",
      title: "Content API and MCP",
      icon: "plug",
      sourcePluginId: "content-access",
      navigationGroupId: "plugins",
      description:
        "Allow external clients to read selected vaults through this server.",
      fields: [
        {
          id: id("notice"),
          type: "output",
          presentation: "status",
          title: "Connection",
          emptyLabel: "Ready",
        },
        {
          id: id("enabled"),
          type: "boolean",
          default: false,
          title: "Enable API and MCP access",
          description:
            "Disabling access revokes all credentials and waits for active reads to finish.",
        },
        {
          id: id("status"),
          type: "output",
          presentation: "status",
          title: "Endpoint status",
        },
        {
          id: id("api"),
          type: "output",
          presentation: "copyable",
          title: "Content API address",
          emptyLabel: "Unavailable",
        },
        {
          id: id("mcp"),
          type: "output",
          presentation: "copyable",
          title: "MCP address",
          emptyLabel: "Unavailable",
        },
        {
          id: id("lifetime"),
          type: "output",
          title: "Settings and credential lifetime",
        },
        {
          id: id("refresh"),
          type: "action",
          title: "Refresh access status",
          label: "Refresh status",
          isBusy: () => model.busy,
          run: async () => {
            await model.refresh();
            return { tone: "success", message: "Status refreshed." };
          },
        },
        {
          id: id("label"),
          type: "string",
          default: "CLI client",
          title: "Credential label",
          maxLength: 128,
        },
        {
          id: id("vaults"),
          type: "multi-enum",
          default: [],
          title: "Vaults this credential can read",
          optionsSource: "content-access.vaults",
          minimumItems: 1,
          maximumItems: 20,
          description:
            "Read-only access to saved notes and file listings in the selected vaults.",
        },
        {
          id: id("hours"),
          type: "integer",
          default: 1,
          minimum: 1,
          maximum: 24,
          title: "Credential lifetime (hours)",
        },
        {
          id: id("create"),
          type: "action",
          title: "Create a read-only credential",
          label: "Create credential",
          isDisabled: () => !model.canCreate,
          isBusy: () => model.busy,
          run: async () => {
            await model.create();
            return {
              tone: "success",
              message:
                "Credential created. Copy it now; it is hidden after one minute.",
            };
          },
        },
        {
          id: id("token"),
          type: "output",
          presentation: "copyable",
          title: "One-time credential",
          copyLabel: "credential",
          emptyLabel: "No credential to display",
          description:
            "Store this securely in your client. It is not saved in this browser and cannot be retrieved again.",
        },
        {
          id: id("dismiss"),
          type: "action",
          title: "Hide the one-time credential",
          label: "Dismiss credential",
          isDisabled: () => !model.hasSecret,
          run: () => {
            model.clearSecret();
            return { tone: "success", message: "Credential hidden." };
          },
        },
        {
          id: id("grants"),
          type: "object-grid",
          default: [],
          title: "Issued credentials",
          rowKey: "grant_id",
          allowAdd: false,
          allowRemove: false,
          reorderable: false,
          properties: [
            {
              id: "grant_id",
              title: "ID",
              type: "string",
              readOnly: true,
              presentation: "hidden",
            },
            { id: "label", title: "Label", type: "string", readOnly: true },
            { id: "vaults", title: "Vaults", type: "string", readOnly: true },
            {
              id: "permission",
              title: "Permission",
              type: "string",
              readOnly: true,
            },
            {
              id: "expires",
              title: "Expires around",
              type: "string",
              readOnly: true,
            },
            { id: "state", title: "Status", type: "string", readOnly: true },
          ],
          rowActions: [
            {
              id: "revoke",
              label: "Revoke",
              variant: "destructive",
              disabled: () => model.busy,
              run: async (row) => {
                await model.revoke(String(row.grant_id));
                return { tone: "success", message: "Credential revoked." };
              },
            },
          ],
        },
      ],
    },
  };
}
