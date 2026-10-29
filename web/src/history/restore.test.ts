import { expect, it, vi } from "vitest";
import type { HistoryClient } from "@lapismd/jj-history";
import type {
  RepositoryRestoreCommand,
  RepositoryRestoreValue,
} from "../../../crates/lapis-app-protocol/bindings/protocol";
import { createHostedRestore } from "./restore";
import { createHostedRepository } from "./services";

const preview = {
  checkpoint: "checkpoint",
  path: "Note.md",
  expected: "exact",
  bytes: 12,
  deleted: false,
};
const capture = {
  changed: false,
  checkpoint: null,
  operation: "operation",
  working_copy: "commit",
};
const request = {
  request_id: "original",
  checkpoint: "checkpoint",
  path: "Note.md",
  expected: "exact",
};

it("preserves all five retained restore actions without inventing identities", async () => {
  const call = vi.fn(
    async (_command: RepositoryRestoreCommand) =>
      capture as RepositoryRestoreValue,
  );
  const service = createHostedRestore(call);
  call.mockResolvedValueOnce(preview);
  expect(await service.preview("checkpoint", "Note.md")).toEqual(preview);
  await service.apply(request);
  call.mockResolvedValueOnce({
    checkpoint: "checkpoint",
    expected: "tree",
    added: 1,
    modified: 2,
    deleted: 3,
  });
  await service.previewVault!("checkpoint");
  const vaultRequest = {
    request_id: "vault-original",
    checkpoint: "checkpoint",
    expected: "tree",
  };
  await service.applyVault!(vaultRequest);
  await service.keepCurrent!();
  expect(call.mock.calls.map(([action]) => action)).toEqual([
    { kind: "preview", checkpoint: "checkpoint", path: "Note.md" },
    { kind: "apply", request },
    { kind: "preview_vault", checkpoint: "checkpoint" },
    { kind: "apply_vault", request: vaultRequest },
    { kind: "keep_current" },
  ]);
});

it("rejects mismatched and malformed native file and vault previews", async () => {
  const call = vi.fn(
    async (_command: RepositoryRestoreCommand) =>
      preview as RepositoryRestoreValue,
  );
  const service = createHostedRestore(call);
  for (const value of [
    { ...preview, checkpoint: "other" },
    { ...preview, path: "Other.md" },
    { ...preview, bytes: -1 },
    { ...preview, bytes: NaN },
    { ...preview, deleted: 1 },
    { ...preview, expected: null },
    null,
  ]) {
    call.mockResolvedValueOnce(value as unknown as RepositoryRestoreValue);
    await expect(service.preview("checkpoint", "Note.md")).rejects.toThrow(
      "Invalid restore preview",
    );
  }
  for (const value of [
    {
      checkpoint: "other",
      expected: "tree",
      added: 0,
      modified: 0,
      deleted: 0,
    },
    {
      checkpoint: "checkpoint",
      expected: "tree",
      added: 0.5,
      modified: 0,
      deleted: 0,
    },
    {
      checkpoint: "checkpoint",
      expected: "tree",
      added: 0,
      modified: -1,
      deleted: 0,
    },
    null,
  ]) {
    call.mockResolvedValueOnce(value as unknown as RepositoryRestoreValue);
    await expect(service.previewVault!("checkpoint")).rejects.toThrow(
      "Invalid vault restore preview",
    );
  }
});

it("fences new restores while workspace close drains accepted observation", async () => {
  let deliver!: (value: RepositoryRestoreValue) => void;
  const call = vi.fn(
    (_command: RepositoryRestoreCommand) =>
      new Promise<RepositoryRestoreValue>((resolve) => {
        deliver = resolve;
      }),
  );
  const service = createHostedRepository(
    { close: vi.fn() } as unknown as HistoryClient,
    createHostedRestore(call),
  );
  const pending = service.restore!.apply(request);
  await vi.waitFor(() => expect(call).toHaveBeenCalledOnce());
  let closed = false;
  const closing = service.close("close").then(() => {
    closed = true;
  });
  await expect(service.restore!.apply(request)).rejects.toThrow("closed");
  expect(closed).toBe(false);
  deliver(capture);
  await pending;
  await closing;
  expect(closed).toBe(true);
  expect(call).toHaveBeenCalledOnce();
});
