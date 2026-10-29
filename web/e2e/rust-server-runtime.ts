import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { createServer } from "node:net";

export async function unusedLoopbackPort(): Promise<number> {
  const listener = createServer();
  await new Promise<void>((resolve, reject) => {
    listener.once("error", reject);
    listener.listen(0, "127.0.0.1", resolve);
  });
  const address = listener.address();
  if (!address || typeof address === "string") throw new Error("No test port");
  await new Promise<void>((resolve) => listener.close(() => resolve()));
  return address.port;
}

/** Real Rust host plus the built retained renderer, with test-owned storage. */
export class RustServerRuntime {
  #children: Array<{
    child: ChildProcessWithoutNullStreams;
    exit: Promise<number | null>;
  }> = [];
  #logs = "";
  readonly vaultId = "3a65a94f-b875-4143-b1f5-a6a32d861092";
  constructor(
    readonly root: string,
    readonly repository: string,
    readonly origin: string,
    readonly password: string,
    readonly agentsConfig?: string,
    readonly operatorTimezone?: string,
    readonly staticRoot: string | undefined = process.env
      .LAPIS_TEST_RUST_STATIC_ROOT,
  ) {}
  get logs(): string {
    return this.#logs;
  }
  get processCommands(): readonly string[] {
    return this.#children.map(({ child }) => child.spawnfile);
  }
  get vault(): string {
    return join(this.root, "vault");
  }

  #spawn(command: string, args: string[], cwd: string, env: NodeJS.ProcessEnv) {
    const child = spawn(command, args, { cwd, env, stdio: "pipe" });
    const log = (chunk: unknown) => {
      this.#logs = (this.#logs + String(chunk)).slice(-32_768);
    };
    child.stdout.on("data", log);
    child.stderr.on("data", log);
    child.on("error", (error) => log(error.message));
    child.stdin.end();
    this.#children.push({
      child,
      exit: new Promise((resolve) => child.once("close", resolve)),
    });
    return child;
  }

  async start(): Promise<void> {
    this.#logs = "";
    await mkdir(this.vault, { recursive: true });
    await writeFile(
      join(this.vault, "Note.md"),
      "# Native hosted note\n\nRead and edit this note through Rust.\n",
      { flag: "wx" },
    ).catch((error: NodeJS.ErrnoException) => {
      if (error.code !== "EEXIST") throw error;
    });
    const secret = join(this.root, "bootstrap-password");
    await writeFile(secret, this.password, { mode: 0o600 });
    const config = join(this.root, "config.json");
    await writeFile(
      config,
      JSON.stringify({
        vaults: [{ id: this.vaultId, name: "Rust vault", root: this.vault }],
      }),
    );
    const environment: NodeJS.ProcessEnv = {
      ...process.env,
      LAPIS_SERVER_DATA_ROOT: join(this.root, "profile"),
      LAPIS_SERVER_ORIGIN: this.origin,
      LAPIS_SERVER_INSECURE_COOKIE: "1",
      LAPIS_SERVER_BOOTSTRAP_USERNAME: "owner",
      LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE: secret,
      LAPIS_SERVER_BUILD_ID: "rust-browser-acceptance",
    };
    if (this.operatorTimezone) environment.TZ = this.operatorTimezone;
    delete environment.LAPIS_SERVER_TOKEN;
    if (this.agentsConfig)
      environment.LAPIS_SERVER_AGENTS_CONFIG = this.agentsConfig;
    else delete environment.LAPIS_SERVER_AGENTS_CONFIG;
    if (this.staticRoot) environment.LAPIS_SERVER_STATIC_ROOT = this.staticRoot;
    else delete environment.LAPIS_SERVER_STATIC_ROOT;
    const service = this.#spawn(
      join(this.repository, ".local/rust-core-target/debug/lapis-server"),
      [
        config,
        this.staticRoot
          ? `127.0.0.1:${new URL(this.origin).port}`
          : "127.0.0.1:0",
      ],
      this.repository,
      environment,
    );
    const address = await new Promise<string>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`Rust service did not start\n${this.logs}`)),
        15_000,
      );
      const data = () => {
        const match = /Lapis file service listening on ([^\s]+)/u.exec(
          this.#logs,
        );
        if (match) {
          clearTimeout(timer);
          service.stdout.off("data", data);
          resolve(`http://${match[1]}`);
        }
      };
      service.stdout.on("data", data);
      service.once("exit", () => {
        clearTimeout(timer);
        reject(new Error(`Rust service exited during startup\n${this.logs}`));
      });
    });
    if (!this.staticRoot) {
      this.#spawn(
        process.execPath,
        [
          join(this.repository, "packages/web/node_modules/vite/bin/vite.js"),
          "preview",
          "--mode",
          "server-rust",
          "--host",
          "127.0.0.1",
          "--port",
          new URL(this.origin).port,
        ],
        join(this.repository, "packages/web"),
        { ...process.env, LAPIS_RUST_SERVER_URL: address },
      );
    }
    const until = Date.now() + 15_000;
    while (Date.now() < until) {
      try {
        if ((await fetch(this.origin)).ok) return;
      } catch {
        /* Wait for the owned preview. */
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error(`Built renderer did not start\n${this.logs}`);
  }

  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }

  async stop(): Promise<void> {
    for (const { child, exit } of this.#children.reverse()) {
      if (child.exitCode === null && child.signalCode === null)
        child.kill("SIGTERM");
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        const code = await Promise.race([
          exit,
          new Promise<never>((_, reject) => {
            timer = setTimeout(() => {
              child.kill("SIGKILL");
              reject(
                new Error(
                  `Test host ${child.spawnfile} exceeded its shutdown deadline\n${this.logs}`,
                ),
              );
            }, 15_000);
          }),
        ]);
        if (child.spawnfile.endsWith("lapis-server") && code !== 0)
          throw new Error(`Rust host did not exit cleanly\n${this.logs}`);
      } finally {
        clearTimeout(timer);
      }
    }
    this.#children = [];
  }
}
