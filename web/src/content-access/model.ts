import type {
  ContentHostStatus,
  CreatedContentGrant,
  ManagedVault,
} from "../../../crates/lapis-app-protocol/bindings/protocol";
import { RustHttpClient } from "../hosted/rust/client";

const prefix = "content-access.";
/** Server-owned settings source. Values and one-time secrets never use App persistence. */
export class ContentAccessModel {
  status?: ContentHostStatus;
  vaults: ManagedVault[] = [];
  busy = false;
  notice = "Loading content access…";
  #label = "CLI client";
  #selected: string[];
  #hours = 1;
  #token = "";
  #expires = new Map<string, number>();
  #listeners = new Set<(id?: string) => void>();
  #abort = new AbortController();
  #closed = false;
  #secretTimer?: ReturnType<typeof setTimeout>;

  constructor(
    readonly host: RustHttpClient,
    vaultId: string,
  ) {
    this.#selected = [vaultId];
  }
  get hasSecret() {
    return this.#token !== "";
  }
  get available() {
    return Boolean(this.status?.available);
  }
  get enabled() {
    return Boolean(this.status?.access.enabled);
  }
  get canCreate() {
    return (
      this.enabled &&
      !this.busy &&
      this.#selected.length > 0 &&
      this.#label.trim().length > 0
    );
  }
  get options() {
    return this.vaults
      .filter((v) => v.archived_at == null)
      .map((v) => ({ value: v.vault_id, label: v.name }));
  }
  async vaultOptions() {
    try {
      const vaults = await this.host.json<ManagedVault[]>(
        "/vaults",
        "GET",
        undefined,
        this.#abort.signal,
      );
      return this.#closed
        ? []
        : vaults
            .filter((v) => v.archived_at == null)
            .map((v) => ({ value: v.vault_id, label: v.name }));
    } catch {
      if (!this.#closed) {
        this.notice =
          "Vault choices could not be loaded. Reopen these settings to retry.";
        this.#notify();
      }
      return [];
    }
  }
  subscribe = (listener: (id?: string) => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };
  #notify() {
    for (const listener of this.#listeners) listener();
  }
  get = (id: string): unknown => {
    switch (id.slice(prefix.length)) {
      case "notice":
        return this.notice;
      case "enabled":
        return this.enabled;
      case "status":
        return this.status
          ? this.available
            ? this.enabled
              ? "Enabled · read-only"
              : "Disabled"
            : "Unavailable for this server address"
          : this.notice;
      case "api":
        return this.status?.api_url ?? "";
      case "mcp":
        return this.status?.mcp_url ?? "";
      case "lifetime":
        return this.status?.persistent
          ? "The enable setting is saved. Credentials expire and become invalid on server restart."
          : "This server keeps settings and credentials only until restart.";
      case "label":
        return this.#label;
      case "vaults":
        return this.#selected;
      case "hours":
        return this.#hours;
      case "token":
        return this.#token;
      case "grants":
        return (this.status?.access.grants ?? []).map((grant) => ({
          grant_id: grant.grant_id,
          label: grant.label,
          vaults: grant.vault_ids
            .map((id) => this.vaults.find((v) => v.vault_id === id)?.name ?? id)
            .join(", "),
          permission: "Read notes",
          expires: new Date(
            this.#expires.get(grant.grant_id) ?? Date.now(),
          ).toLocaleString(),
          state: grant.active ? "Active" : "Expired or revoked",
        }));
      default:
        return undefined;
    }
  };
  set = async (id: string, value: unknown) => {
    if (this.#closed || this.busy)
      throw new Error("Wait for the current content request to finish.");
    switch (id.slice(prefix.length)) {
      case "enabled": {
        if (typeof value !== "boolean")
          throw new Error("Choose enabled or disabled.");
        if (value && !this.available)
          throw new Error(
            "Use an HTTPS server address or a loopback address to enable access.",
          );
        await this.#run(async () => {
          this.clearSecret();
          await this.#json("", "POST", { enabled: value });
          await this.#load();
        });
        break;
      }
      case "label": {
        if (
          typeof value !== "string" ||
          new TextEncoder().encode(value.trim()).length > 128 ||
          /\p{Cc}/u.test(value)
        )
          throw new Error(
            "Enter a label of at most 128 UTF-8 bytes without control characters.",
          );
        this.#label = value;
        break;
      }
      case "vaults": {
        if (
          !Array.isArray(value) ||
          value.length < 1 ||
          value.length > 20 ||
          value.some(
            (id) =>
              typeof id !== "string" ||
              !this.options.some((v) => v.value === id),
          )
        )
          throw new Error("Select one or more active vaults.");
        this.#selected = [...new Set(value as string[])];
        break;
      }
      case "hours": {
        if (
          typeof value !== "number" ||
          !Number.isInteger(value) ||
          value < 1 ||
          value > 24
        )
          throw new Error("Choose 1 to 24 hours.");
        this.#hours = value;
        break;
      }
      default:
        throw new Error("This setting is read-only.");
    }
    this.#notify();
    return this.get(id);
  };
  async #json<T>(path: string, method = "GET", value?: unknown): Promise<T> {
    return this.host.json(
      `/content-access${path}`,
      method,
      value,
      this.#abort.signal,
    );
  }
  async #load() {
    const [status, vaults] = await Promise.all([
      this.#json<ContentHostStatus>(""),
      this.host.json<ManagedVault[]>(
        "/vaults",
        "GET",
        undefined,
        this.#abort.signal,
      ),
    ]);
    if (this.#closed) return;
    this.status = status;
    this.vaults = vaults;
    this.#selected = this.#selected.filter((id) =>
      this.options.some((v) => v.value === id),
    );
    this.#expires = new Map(
      status.access.grants.map((g) => [
        g.grant_id,
        Date.now() + g.expires_in_seconds * 1000,
      ]),
    );
  }
  async #run(action: () => Promise<void>) {
    if (this.#closed || this.busy)
      throw new Error("Wait for the current content request to finish.");
    this.busy = true;
    this.#notify();
    try {
      await action();
      this.notice = "";
    } catch {
      this.notice =
        "Content request outcome is unavailable. Refresh status before retrying.";
      throw new Error(this.notice);
    } finally {
      this.busy = false;
      if (!this.#closed) this.#notify();
    }
  }
  refresh = () => this.#run(() => this.#load());
  create = () =>
    this.#run(async () => {
      if (!this.enabled || !this.#selected.length || !this.#label.trim())
        throw new Error("Enable access and select vaults first.");
      this.clearSecret();
      const result = await this.#json<CreatedContentGrant>("/grants", "POST", {
        label: this.#label.trim(),
        vault_ids: this.#selected,
        lifetime_seconds: this.#hours * 3600,
      });
      if (this.#closed) return;
      this.#token = result.token;
      this.#secretTimer = setTimeout(() => this.clearSecret(), 60_000);
      await this.#load();
    });
  revoke = (id: string) =>
    this.#run(async () => {
      this.clearSecret();
      await this.#json(`/grants/${encodeURIComponent(id)}`, "DELETE");
      await this.#load();
    });
  clearSecret = () => {
    clearTimeout(this.#secretTimer);
    this.#token = "";
    this.#notify();
  };
  dispose() {
    this.#closed = true;
    this.#abort.abort();
    this.clearSecret();
    this.#listeners.clear();
  }
}
