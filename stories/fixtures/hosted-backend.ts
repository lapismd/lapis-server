import { contentAccessPlugin } from "../../web/src/content-access/plugin";
import { contentFixture } from "./content-access";
import { MemoryAppDatabase, MemoryVaultAdapter } from "@lapis-notes/api";
import type { NotesWorkspaceHost } from "@lapis-notes/app-profile/workspace-session";
import type { HostedBackend, HostedVault } from "../../web/src/hosted/backend";

export type HostedScenario =
  | "login"
  | "loading"
  | "protocol"
  | "empty"
  | "vaults"
  | "opening"
  | "startup-failure";

const fixtureGates = new WeakMap<HostedBackend, () => void>();
export function releaseHostedFixture(backend: HostedBackend) {
  fixtureGates.get(backend)?.();
}

export function createHostedFixture(scenario: HostedScenario) {
  let authenticated = !["login", "loading", "protocol"].includes(scenario);
  let vaults: HostedVault[] =
    scenario === "empty"
      ? []
      : [
          { id: "notes", name: "Notes", role: "owner" },
          { id: "archive", name: "Archive", role: "owner", archivedAt: 1 },
        ];
  const content = contentFixture(() => vaults);
  let releaseGate!: () => void;
  const gate = new Promise<void>((done) => {
    releaseGate = done;
  });
  const adapters = new Map<string, MemoryVaultAdapter>();
  let failed = false;
  const backend: HostedBackend = {
    runtime: "rust-server",
    protocolMajor: 3,
    capabilities: { manageVaults: true, transferVaults: true },
    info: async () => {
      if (scenario === "loading") await gate;
      return {
        buildId: "catalog",
        protocolMajor: scenario === "protocol" ? 4 : 3,
      };
    },
    current: async () => {
      if (!authenticated) throw new Error("Sign in required");
    },
    login: async (username, password) => {
      if (username !== "owner" || password !== "fixture-password")
        throw new Error("Invalid username or password");
      authenticated = true;
    },
    logout: async () => {
      authenticated = false;
    },
    vaults: async () => [...vaults],
    create: async (name) => {
      const vaultId = `vault-${vaults.length}`;
      vaults = [...vaults, { id: vaultId, name, role: "owner" }];
      return { vaultId };
    },
    rename: async (id, name) => {
      vaults = vaults.map((vault) =>
        vault.id === id ? { ...vault, name } : vault,
      );
    },
    archive: async (id) => {
      vaults = vaults.map((vault) =>
        vault.id === id ? { ...vault, archivedAt: 1 } : vault,
      );
    },
    restore: async (id) => {
      vaults = vaults.map((vault) =>
        vault.id === id ? { ...vault, archivedAt: undefined } : vault,
      );
    },
    importArchive: async (name) => {
      await backend.create(name);
    },
    exportUrl: () => "/catalog-vault.zip",
    prepare: async (profile) => {
      if (scenario === "opening") await gate;
      let adapter = adapters.get(profile.id);
      if (!adapter) {
        adapter = new MemoryVaultAdapter({
          "Welcome.md": "# Welcome\n\nThe production Notes workspace.\n",
        });
        adapters.set(profile.id, adapter);
      }
      const database = new MemoryAppDatabase(adapter.getVaultId());
      const workspaceHost: NotesWorkspaceHost = {
        configure: async (app) => {
          if (scenario === "startup-failure" && !failed) {
            failed = true;
            throw new Error("Catalog workspace configuration failed");
          }
          app.plugins.registerStaticPlugins([
            {
              plugin: contentAccessPlugin(content, profile.id),
              required: true,
              enabledByDefault: true,
            },
          ]);
        },
      };
      return {
        adapter,
        database: {
          id: "catalog-memory",
          canOpen: () => true,
          open: async () => {
            await database.open();
            return database;
          },
        },
        workspaceHost,
        release: async () => {},
      };
    },
  };
  fixtureGates.set(backend, releaseGate);
  return { backend, releaseGate, adapters };
}
