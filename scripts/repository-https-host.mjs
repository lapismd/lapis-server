import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

/** Actual compiled host; every path, owner and credential belongs to this test. */
export class ColdHttpsHost {
  ids = [randomUUID(), randomUUID()];
  token = randomBytes(32).toString("base64url");
  logs = "";
  constructor(base, environment) {
    this.base = base;
    this.environment = environment;
    this.key = join(base, "repository-key");
    this.profile = join(base, "server-profile");
    this.vaults = [join(base, "original"), join(base, "backup")];
    for (const path of [this.profile, ...this.vaults])
      mkdirSync(path, { recursive: true, mode: 0o700 });
    writeFileSync(this.key, randomBytes(32), { mode: 0o600 });
    this.password = join(base, "bootstrap-password");
    writeFileSync(this.password, randomBytes(32).toString("base64url"), {
      mode: 0o600,
    });
    this.config = join(base, "server.json");
    writeFileSync(
      this.config,
      JSON.stringify({
        vaults: this.vaults.map((root, i) => ({
          id: this.ids[i],
          name: `Fixture ${i}`,
          root,
        })),
      }),
    );
  }
  async start(key = this.key) {
    const environment = {
      ...this.environment,
      LAPIS_SERVER_TOKEN: this.token,
      LAPIS_SERVER_DATA_ROOT: this.profile,
      LAPIS_SERVER_ORIGIN: "https://fixture.invalid",
      LAPIS_SERVER_BOOTSTRAP_USERNAME: "owner",
      LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE: this.password,
      LAPIS_SERVER_REPOSITORY_KEY_FILE: key,
    };
    if (!key) delete environment.LAPIS_SERVER_REPOSITORY_KEY_FILE;
    const binary = resolve(".local/rust-core-target/debug/lapis-server");
    this.child = spawn(binary, [this.config, "127.0.0.1:0"], {
      env: environment,
      stdio: ["ignore", "pipe", "pipe"],
    });
    this.closed = new Promise((resolve) => this.child.once("exit", resolve));
    await new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(Error("Cold Server startup timed out")),
        15_000,
      );
      const record = (bytes) => {
        const value = bytes.toString();
        this.logs = (this.logs + value).slice(-32768);
        const match = /Lapis file service listening on ([^\s]+)/u.exec(value);
        if (match) {
          clearTimeout(timer);
          this.url = `http://${match[1]}`;
          resolve();
        }
      };
      this.child.stdout.on("data", record);
      this.child.stderr.on("data", record);
      this.child.once("error", (error) => {
        clearTimeout(timer);
        reject(error);
      });
      this.child.once("exit", () => {
        clearTimeout(timer);
        reject(Error("Cold Server exited before readiness"));
      });
    });
  }
  async stop() {
    if (!this.child || this.child.exitCode !== null) return;
    this.child.kill("SIGTERM");
    let timer;
    await Promise.race([
      this.closed,
      new Promise((_, reject) => {
        timer = setTimeout(() => {
          this.child.kill("SIGKILL");
          reject(Error("Cold Server failed to drain"));
        }, 10_000);
      }),
    ]).finally(() => clearTimeout(timer));
  }
  async request(path, body, method = body ? "POST" : "GET") {
    return fetch(this.url + path, {
      method,
      headers: {
        authorization: `Bearer ${this.token}`,
        "x-lapis-protocol": "3",
        "content-type": "application/json",
      },
      body: body ? JSON.stringify(body) : undefined,
    });
  }
  async json(path, body) {
    const response = await this.request(path, body);
    assert.equal(
      response.ok,
      true,
      `Native request refused: ${response.status}`,
    );
    return response.json();
  }
  async attach(index = 0) {
    return this.json(`/api/v3/vaults/${this.ids[index]}/clients`, {
      protocol_version: 3,
    });
  }
  route(client, endpoint = "repository-remotes") {
    return `/api/v3/clients/${client.client_id}/${endpoint}?vault=${client.vault_id}&generation=${client.generation}`;
  }
  async settings(client) {
    return this.json(this.route(client));
  }
  async command(client, kind, fields = {}) {
    const current = await this.settings(client);
    return this.json(this.route(client), {
      kind,
      expectedRevision: current.revision,
      ...fields,
    });
  }
}
