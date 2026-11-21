import { writable } from "svelte/store";

export type ConnectionPhase =
  | "reconnecting"
  | "connected"
  | "offline"
  | "sign-in"
  | "update-required";
type ProbeResult = Exclude<ConnectionPhase, "offline" | "reconnecting">;
type Snapshot = { phase: ConnectionPhase; lastContact?: number };
type Probe = (signal: AbortSignal) => Promise<ProbeResult>;

const transportFailures = new WeakSet<TypeError>();

export function recordHostedTransportFailure(error: TypeError): void {
  transportFailures.add(error);
}

export class HostedTransportError extends TypeError {
  constructor(cause: unknown) {
    super(
      cause instanceof Error ? cause.message : "Lapis Server is unreachable",
      { cause },
    );
  }
}

export function isHostedTransportError(error: unknown): boolean {
  return (
    error instanceof HostedTransportError ||
    (error instanceof TypeError &&
      (transportFailures.has(error) ||
        /failed to fetch|load failed|network\s?error/i.test(error.message)))
  );
}

export class HostedConnection {
  #snapshot: Snapshot = { phase: "reconnecting" };
  #store = writable(this.#snapshot);
  #probe?: Probe;
  #flight?: Promise<void>;
  #controller?: AbortController;
  #timer?: ReturnType<typeof setTimeout>;
  #delay = 1000;
  readonly subscribe = this.#store.subscribe;

  get phase() {
    return this.#snapshot.phase;
  }

  reached(phase: ProbeResult): void {
    this.#delay = 1000;
    clearTimeout(this.#timer);
    this.#timer = undefined;
    this.#set({ phase, lastContact: Date.now() });
  }

  lost(): void {
    this.#set({ ...this.#snapshot, phase: "offline" });
    this.#schedule();
  }

  suspect(): void {
    if (this.phase === "connected") void this.retry();
  }

  start(probe: Probe): () => void {
    this.#probe = probe;
    const disconnect = () => {
      this.#controller?.abort();
      this.lost();
    };
    const retry = () => {
      void this.retry();
    };
    const visible = () => {
      if (document.visibilityState === "visible" && this.phase !== "connected")
        retry();
    };
    window.addEventListener("offline", disconnect);
    window.addEventListener("online", retry);
    document.addEventListener("visibilitychange", visible);
    return () => {
      this.#probe = undefined;
      this.#controller?.abort();
      clearTimeout(this.#timer);
      this.#timer = undefined;
      window.removeEventListener("offline", disconnect);
      window.removeEventListener("online", retry);
      document.removeEventListener("visibilitychange", visible);
    };
  }

  retry(): Promise<void> {
    if (!this.#probe || this.#flight) return this.#flight ?? Promise.resolve();
    clearTimeout(this.#timer);
    this.#timer = undefined;
    const probe = this.#probe;
    const controller = new AbortController();
    this.#controller = controller;
    this.#set({ ...this.#snapshot, phase: "reconnecting" });
    const signal = AbortSignal.any([
      controller.signal,
      AbortSignal.timeout(5000),
    ]);
    this.#flight = Promise.resolve()
      .then(() => probe(signal))
      .then(
        (phase) => {
          if (!controller.signal.aborted) this.reached(phase);
        },
        () => {
          if (!controller.signal.aborted) this.lost();
        },
      )
      .finally(() => {
        this.#flight = undefined;
        this.#controller = undefined;
      });
    return this.#flight;
  }

  #set(snapshot: Snapshot): void {
    this.#snapshot = snapshot;
    this.#store.set(snapshot);
  }

  #schedule(): void {
    if (!this.#probe || this.#timer) return;
    this.#timer = setTimeout(() => {
      this.#timer = undefined;
      void this.retry();
    }, this.#delay);
    this.#delay = Math.min(this.#delay * 2, 30_000);
  }
}
