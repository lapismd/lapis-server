import type { PluginSettingsDefinition } from "@lapis-notes/api";
import CredentialVaultSetting, {
  type CredentialVaultField,
} from "./CredentialVaultSetting.svelte";
import { personalTokenLifetimes, type ContentAccessModel } from "./model";

type SettingField = NonNullable<
  PluginSettingsDefinition["section"]["fields"]
>[number];

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
      navigationGroupId: "core-plugins",
      description:
        "Connect external clients to selected vaults and explicitly permitted operations.",
      fields: [
        {
          id: "content-endpoints",
          type: "group",
          title: "Endpoints",
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
                "Disabling access revokes all credentials and waits for active operations to finish.",
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
              layout: "full-width",
              showFullValue: true,
              title: "Content API address",
              emptyLabel: "Unavailable",
            },
            {
              id: id("mcp"),
              type: "output",
              presentation: "copyable",
              layout: "full-width",
              showFullValue: true,
              title: "MCP address",
              emptyLabel: "Unavailable",
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
          ],
        },
        {
          id: "personal-access",
          type: "group",
          title: "Personal access tokens",
          description:
            "Use a personal token for the Content API or MCP. Tokens survive a server restart.",
          fields: [
            ...credentialFields(model, true),
            {
              id: id("personal-tokens"),
              type: "object-grid",
              default: [],
              title: "Issued personal tokens",
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
                {
                  id: "vaults",
                  title: "Vaults",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "permission",
                  title: "Permission",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "expires",
                  title: "Expires",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "last_used",
                  title: "Last used",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "state",
                  title: "Status",
                  type: "string",
                  readOnly: true,
                },
              ],
              rowActions: [
                {
                  id: "revoke-personal",
                  label: "Revoke",
                  variant: "destructive",
                  disabled: (row) => model.busy || row.state !== "Active",
                  run: async (row) => {
                    await model.revoke(String(row.grant_id), true);
                    return {
                      tone: "success",
                      message: "Personal access token revoked.",
                    };
                  },
                },
              ],
            },
          ],
        },
        {
          id: "mcp-oauth",
          type: "group",
          title: "MCP OAuth",
          description:
            "Connect an MCP client using your Lapis Server account and approve the vaults it can access.",
          fields: [
            {
              id: id("mcp-connections"),
              type: "object-grid",
              default: [],
              title: "Connected applications",
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
                {
                  id: "label",
                  title: "Application",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "vaults",
                  title: "Vaults",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "permission",
                  title: "Permission",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "expires",
                  title: "Expires",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "last_used",
                  title: "Last used",
                  type: "string",
                  readOnly: true,
                },
                {
                  id: "state",
                  title: "Status",
                  type: "string",
                  readOnly: true,
                },
              ],
              rowActions: [
                {
                  id: "disconnect-mcp",
                  label: "Disconnect",
                  variant: "destructive",
                  disabled: (row) => model.busy || row.state !== "Active",
                  run: async (row) => {
                    await model.disconnect(String(row.grant_id));
                    return {
                      tone: "success",
                      message: "Application disconnected.",
                    };
                  },
                },
              ],
            },
          ],
        },
        {
          id: "temporary-access",
          type: "group",
          title: "Temporary credentials",
          description:
            "Temporary credentials expire within 24 hours or when the server restarts.",
          fields: [
            ...credentialFields(model, false),
            {
              id: id("grants"),
              type: "object-grid",
              default: [],
              title: "Issued temporary credentials",
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
                {
                  id: "vaults",
                  title: "Vaults",
                  type: "string",
                  readOnly: true,
                },
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
                {
                  id: "state",
                  title: "Status",
                  type: "string",
                  readOnly: true,
                },
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
      ],
    },
  };
}

function credentialFields(
  model: ContentAccessModel,
  personal: boolean,
): SettingField[] {
  const id = (name: string) =>
    `content-access.${personal ? "personal-" : ""}${name}`;
  const lifetime: SettingField = personal
    ? {
        id: id("lifetime"),
        type: "enum",
        title: "Personal access token expiry",
        default: "days90",
        options: personalTokenLifetimes,
        description: "Available on servers with a persistent profile.",
      }
    : {
        id: id("hours"),
        type: "integer",
        title: "Credential lifetime (hours)",
        default: 1,
        minimum: 1,
        maximum: 24,
      };

  const vaultField: CredentialVaultField = {
    id: id("vaults"),
    type: "custom",
    component: CredentialVaultSetting,
    title: personal
      ? "Vaults this token can read"
      : "Vaults this credential can read",
    presentation: "full-width",
    contentAccess: model,
    default: [],
  };

  return [
    {
      id: id("label"),
      type: "string",
      default: "CLI client",
      title: personal ? "Token label" : "Credential label",
      maxLength: 128,
    },
    vaultField,
    {
      id: id("permissions"),
      type: "multi-enum",
      default: [],
      title: personal
        ? "Token plugin permissions"
        : "Plugin operation permissions",
      optionsSource: "content-access.permissions",
      description:
        "Optional operations. Write access can change Markdown notes, Tasks, or installed agent skills.",
    },
    lifetime,
    {
      id: id("create"),
      type: "action",
      title: personal
        ? "Create a personal access token"
        : "Create a scoped credential",
      label: personal ? "Create personal access token" : "Create credential",
      isDisabled: () => !model.canCreate(personal),
      isBusy: () => model.busy,
      run: async () => {
        await model.create(personal);
        return {
          tone: "success",
          message: "Copy the secret now. It is hidden after one minute.",
        };
      },
    },
    {
      id: id("token"),
      type: "output",
      presentation: "copyable",
      title: personal
        ? "One-time personal access token"
        : "One-time credential",
      copyLabel: personal ? "personal access token" : "credential",
      emptyLabel: personal ? "No token to display" : "No credential to display",
      description:
        "Copy the secret to your client. It cannot be retrieved again.",
    },
    {
      id: id("dismiss"),
      type: "action",
      title: personal
        ? "Hide the one-time token"
        : "Hide the one-time credential",
      label: personal ? "Dismiss token" : "Dismiss credential",
      isDisabled: () => !model.get(id("token")),
      run: () => {
        model.clearSecret();
        return { tone: "success", message: "Secret hidden." };
      },
    },
  ];
}
