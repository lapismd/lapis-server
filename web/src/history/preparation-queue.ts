import type { Handle, HistoryClient } from "@lapismd/jj-history";

type Reader = Pick<HistoryClient, "open" | "close" | "status">;
type Preparation = {
  handle: Handle;
  reader: Reader;
  closing?: Promise<void>;
};

function pause(signal: AbortSignal): Promise<void> {
  if (signal.aborted) return Promise.reject(signal.reason);
  return new Promise((resolve, reject) => {
    const cancel = () => {
      clearTimeout(timer);
      reject(signal.reason);
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", cancel);
      resolve();
    }, 50);
    signal.addEventListener("abort", cancel, { once: true });
  });
}

/** One hosted preparation at a time. The history plugin owns its own queue. */
export class PreparationQueue {
  readonly #reader: Reader;
  readonly #abort = new AbortController();
  #tail: Promise<unknown> = Promise.resolve();
  #previous?: Preparation;
  constructor(reader: Reader) {
    this.#reader = reader;
  }
  open(
    operation: Parameters<Reader["open"]>[0],
    reader: Reader = this.#reader,
  ): Promise<Handle> {
    const job = this.#tail.then(async () => {
      this.#abort.signal.throwIfAborted();
      const previous = this.#previous;
      if (previous) await this.#available(previous);
      this.#abort.signal.throwIfAborted();
      const handle = await reader.open(operation);
      this.#previous = { handle, reader };
      return handle;
    });
    this.#tail = job.catch(() => {});
    return job;
  }
  close(handle: Handle, reader: Reader = this.#reader): Promise<void> {
    const job = reader.close(handle);
    if (this.#previous?.handle === handle) this.#previous.closing = job;
    return job;
  }
  stop(): void {
    this.#abort.abort(new Error("Repository history is closed"));
  }
  async #available(previous: Preparation): Promise<void> {
    for (let attempt = 0; attempt < 100; attempt++) {
      this.#abort.signal.throwIfAborted();
      if (previous.closing) return previous.closing;
      try {
        const status = await previous.reader.status(previous.handle);
        if (previous.closing) return previous.closing;
        if (status.state !== "preparing") return;
      } catch (error) {
        if (previous.closing) return previous.closing;
        throw error;
      }
      await pause(this.#abort.signal);
    }
    throw new Error(
      "Repository preparation is still running; try again shortly",
    );
  }
}
