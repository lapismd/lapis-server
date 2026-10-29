import { expect, it, vi } from "vitest";
import type { Handle, HistoryClient } from "@lapismd/jj-history";
import { createHostedRepository } from "./services";

function reader(name: string) {
  return {
    heads: vi.fn(async () => ({ heads: [name] })),
    open: vi.fn(async () => `${name}-handle` as Handle),
    close: vi.fn(async (_handle: Handle) => {}),
    status: vi.fn(async () => ({ state: "ready" })),
    read: vi.fn(async () => name),
    blame: vi.fn(async () => name),
  };
}
function fixture() {
  const managed = reader("managed"),
    original = reader("original");
  const source = {
    discover: vi.fn(async () => ({ original: true })),
    open: vi.fn(async () => original as unknown as HistoryClient),
  };
  return {
    managed,
    original,
    source,
    services: createHostedRepository(
      managed as unknown as HistoryClient,
      undefined,
      undefined,
      source,
    ),
  };
}

it("discovers original sources lazily and keeps source reads and handles independent", async () => {
  const f = fixture();
  expect(f.source.discover).not.toHaveBeenCalled();
  expect(f.source.open).not.toHaveBeenCalled();
  expect(await f.services.sources!()).toEqual({ original: true });
  expect(f.source.open).not.toHaveBeenCalled();
  await Promise.all([
    f.services.original!.heads(),
    f.services.original!.heads(),
  ]);
  expect(f.source.open).toHaveBeenCalledOnce();
  const managed = await f.services.reader.open("managed-operation");
  const original = await f.services.original!.open("original-operation");
  expect(managed).not.toBe(original);
  await f.services.reader.read(managed, { kind: "query_info" });
  await f.services.original!.read(original, { kind: "query_info" });
  await f.services.original!.blame!(original, {} as never);
  expect(f.managed.read).toHaveBeenCalledWith(managed, { kind: "query_info" });
  expect(f.original.read).toHaveBeenCalledWith(original, {
    kind: "query_info",
  });
  expect(f.managed.blame).not.toHaveBeenCalled();
  await f.services.original!.close(original);
  expect(f.original.close).toHaveBeenCalledExactlyOnceWith(original);
  expect(f.managed.close).not.toHaveBeenCalled();
  await f.services.close("close");
  expect(f.managed.close).toHaveBeenCalledExactlyOnceWith(managed);
  expect(f.original.close).toHaveBeenCalledOnce();
});

it("shares preparation capacity across the managed and original clients", async () => {
  const f = fixture();
  f.managed.status.mockResolvedValueOnce({ state: "preparing" });
  const managed = await f.services.reader.open("managed-operation");
  const original = await f.services.original!.open("original-operation");
  expect(f.managed.status).toHaveBeenCalledWith(managed);
  expect(f.original.open).toHaveBeenCalledExactlyOnceWith("original-operation");
  expect(original).toBe("original-handle");
  await f.services.close("close");
});

it("retries a failed original admission without falling back to managed reads", async () => {
  const f = fixture();
  f.source.open.mockRejectedValueOnce(new Error("Original metadata replaced"));
  await expect(f.services.original!.heads()).rejects.toThrow("replaced");
  expect(f.managed.heads).not.toHaveBeenCalled();
  await f.services.original!.heads();
  expect(f.source.open).toHaveBeenCalledTimes(2);
  expect(f.original.heads).toHaveBeenCalledOnce();
  await f.services.close("close");
});

it("drains accepted original opening, fences its reply on close, and admits no new sources", async () => {
  const f = fixture();
  let deliver!: (reader: HistoryClient) => void;
  f.source.open.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        deliver = resolve;
      }),
  );
  const opening = f.services.original!.heads();
  const rejected = expect(opening).rejects.toThrow("closed");
  await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
  const closing = f.services.close("reload");
  await expect(f.services.sources!()).rejects.toThrow("closed");
  await expect(f.services.original!.open("late")).rejects.toThrow("closed");
  deliver(f.original as unknown as HistoryClient);
  await rejected;
  await closing;
  expect(f.original.heads).not.toHaveBeenCalled();
  expect(f.original.open).not.toHaveBeenCalled();
  expect(f.source.open).toHaveBeenCalledOnce();
});

it("retains failed original cleanup for retry through the owning reader", async () => {
  const f = fixture();
  const managed = await f.services.reader.open("managed");
  const original = await f.services.original!.open("original");
  f.original.close.mockRejectedValueOnce(new Error("Lost original close"));
  await expect(f.services.close("close")).rejects.toThrow(
    "Lost original close",
  );
  expect(f.managed.close).toHaveBeenCalledExactlyOnceWith(managed);
  await f.services.close("close");
  expect(f.original.close).toHaveBeenCalledTimes(2);
  expect(f.original.close).toHaveBeenLastCalledWith(original);
  expect(f.managed.close).toHaveBeenCalledOnce();
});
