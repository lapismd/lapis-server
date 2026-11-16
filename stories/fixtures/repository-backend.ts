import { HistoryClient } from "@lapismd/jj-history";
import { parseWorkingCopyStatus } from "@lapis-notes/history/repository/controllers/working-copy-status";
import { createHostedRepository } from "../../web/src/history/services";
import { createHostedRemotes } from "../../web/src/history/remotes";
import type {
  RepositoryRemotesSnapshot,
  RepositorySyncSnapshot,
} from "../../crates/lapis-app-protocol/bindings/protocol";
import { createHostedFixture } from "./hosted-backend";

/** UI composition only: real host adapters with declared in-memory native data. */
export function createRepositoryHostedFixture(
  synchronization?: RepositorySyncSnapshot,
) {
  const fixture = createHostedFixture("vaults");
  const prepare = fixture.backend.prepare;
  fixture.backend.prepare = async (profile) => {
    const resources = await prepare(profile);
    const lifetime = new AbortController();
    const snapshot: RepositoryRemotesSnapshot = {
      revision: 1,
      capabilities: {
        targets: ["backup", "original"],
        anonymousHttps: true,
        httpsTokens: true,
        hostSshAgent: true,
        hostSshDefault: true,
        githubDeviceSignIn: false,
        forgejoTokens: true,
      },
      settings: {
        host: "server",
        storage: "available",
        sshAvailable: true,
        deviceSignInAvailable: false,
        credentials: [],
        automaticSync: false,
        targets: ["backup", "original"].map((target) => ({
          target: target as "backup" | "original",
          connection: {
            name: "origin",
            url: `ssh://git@git.example.test/team/${target}.git`,
            branch: "main",
            bookmark: target === "original" ? "main" : "",
            authentication: "ssh-agent",
            credential: "",
          },
          status: "connected",
          bookmarks: target === "original" ? ["main"] : [],
          canFetch: true,
          canPush: true,
          observation: {
            incoming: 2,
            outgoing: 1,
            fetchedAt: "2026-11-15T12:00:00Z",
          },
        })),
      },
    };
    const reader = () =>
      new HistoryClient(profile.id, {
        invoke: async (command) => {
          if (command !== "heads")
            throw new Error("No catalog history operation is selected");
          return JSON.stringify({
            version: 1,
            data: {
              version: 1,
              profile: "git_sha1_jj045",
              repository: "ab".repeat(16),
              generation: 1,
              heads: [],
            },
          });
        },
      });
    return {
      ...resources,
      repositoryFactory: () =>
        createHostedRepository(
          reader(),
          undefined,
          {
            start: async () => ({
              phase: "ready",
              checkpoint: null,
              error: null,
            }),
            capture: async () => {
              throw new Error("Catalog capture is not selected");
            },
            stop: async () => {},
          },
          {
            discover: async () => ({ original: true }),
            open: async () => reader(),
          },
          {
            remotes: createHostedRemotes(
              async (command) => {
                if (command) {
                  if (
                    command.kind !== "fetch" &&
                    command.kind !== "test_connection"
                  )
                    throw new Error(
                      "This catalog only exercises manual fetch and connection checks",
                    );
                  const target = snapshot.settings.targets.find(
                    (target) => target.target === command.target,
                  )!;
                  if (command.kind === "fetch")
                    target.observation = {
                      incoming: 0,
                      outgoing: 1,
                      fetchedAt: "2026-11-15T12:01:00Z",
                    };
                }
                return structuredClone(snapshot);
              },
              lifetime.signal,
              undefined,
              synchronization
                ? async (command) => {
                    if (!synchronization)
                      throw new Error("Catalog synchronization is unavailable");
                    if (command) {
                      if (
                        command.expected_revision !==
                          synchronization.revision ||
                        command.kind !== "resume_apply" ||
                        command.request_id !== synchronization.pendingApply
                      )
                        throw new Error(
                          "Stale or unsupported catalog recovery request",
                        );
                      synchronization = {
                        ...synchronization,
                        recovery: false,
                        phase: "paused",
                        pendingApply: undefined,
                      };
                    }
                    return structuredClone(synchronization);
                  }
                : undefined,
            ),
            workingCopy: async () =>
              parseWorkingCopyStatus(
                JSON.stringify({
                  version: 1,
                  source: "original",
                  operation: "ab".repeat(64),
                  working_copy: "cd".repeat(20),
                  workspace: "default",
                  commit: {
                    change_id: "ef".repeat(16),
                    description: "Current saved work",
                    author: {
                      name: "Fixture",
                      email: "fixture@example.test",
                      timestamp: { millis: "1794748800000", offset_minutes: 0 },
                    },
                  },
                  files: 3,
                  lines: { additions: 7, deletions: 2 },
                  uncounted: 0,
                }),
              ),
          },
        ),
      release: async () => {
        lifetime.abort();
        await resources.release();
      },
    };
  };
  return fixture;
}
