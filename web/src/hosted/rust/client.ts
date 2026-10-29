import {
  PROTOCOL_MAJOR,
  type HostInfo,
  type ManagedVault,
  type SessionInfo,
} from "../../../../crates/lapis-app-protocol/bindings/protocol";
import { RustVaultClient } from "./vault-client";

export class RustHostError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number,
    readonly details?: unknown,
  ) {
    super(message);
    this.name = "RustHostError";
  }
}

export class RustHttpClient {
  readonly protocolMajor = PROTOCOL_MAJOR;
  #csrfToken?: string;

  constructor(
    readonly baseUrl = "",
    readonly fetch: typeof globalThis.fetch = (...args) =>
      globalThis.fetch(...args),
  ) {}

  url(path: string): string {
    return `${this.baseUrl.replace(/\/$/u, "")}/api/v${PROTOCOL_MAJOR}${path}`;
  }

  async response(path: string, init: RequestInit = {}): Promise<Response> {
    const headers = new Headers(init.headers);
    headers.set("x-lapis-protocol", String(PROTOCOL_MAJOR));
    const method = init.method ?? "GET";
    if (method !== "GET" && method !== "HEAD" && this.#csrfToken)
      headers.set("x-lapis-csrf", this.#csrfToken);
    const response = await this.fetch(this.url(path), {
      ...init,
      headers,
      credentials: "same-origin",
    });
    if (!response.ok) {
      let error: { code?: string; message?: string } = {};
      try {
        const value = await response.json();
        error =
          value?.outcome?.status === "error" ? value.outcome.error : value;
      } catch {
        // A proxy can replace structured failures; never retry a mutation.
      }
      throw new RustHostError(
        error?.code ?? "IO",
        error?.message ?? `Server request failed (${response.status})`,
        response.status,
        error,
      );
    }
    return response;
  }

  async json<T>(path: string, method = "GET", value?: unknown): Promise<T> {
    const response = await this.response(path, {
      method,
      ...(value === undefined
        ? {}
        : {
            headers: { "content-type": "application/json" },
            body: JSON.stringify(value),
          }),
    });
    return response.status === 204 ? (undefined as T) : response.json();
  }

  info(): Promise<HostInfo> {
    return this.json("/system/info");
  }

  async current(): Promise<SessionInfo> {
    const session = await this.json<SessionInfo>("/auth/current");
    this.#csrfToken = session.csrf_token;
    return session;
  }

  async login(username: string, password: string): Promise<void> {
    const session = await this.json<SessionInfo>("/auth/login", "POST", {
      username,
      password,
    });
    this.#csrfToken = session.csrf_token;
  }

  async logout(): Promise<void> {
    await this.json("/auth/logout", "POST");
    this.#csrfToken = undefined;
  }

  vaults(): Promise<ManagedVault[]> {
    return this.json("/vaults");
  }

  async attach(vaultId: string): Promise<RustVaultClient> {
    const descriptor = await this.json(
      `/vaults/${encodeURIComponent(vaultId)}/clients`,
      "POST",
      { protocol_version: PROTOCOL_MAJOR },
    );
    return RustVaultClient.admitted(this, descriptor, vaultId);
  }
}
