import { expect, it, vi } from "vitest";
import { RustHttpClient } from "./client";
import { preparePluginCredentials } from "./plugin-credentials";

it.each([false, true])(
  "discovers credential scopes only on an advertising attachment (%s)",
  async (advertised) => {
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValueOnce(
        Response.json(
          { client_id: "client", vault_id: "vault", generation: "generation" },
          { headers: advertised ? { "x-lapis-plugin-credentials": "1" } : {} },
        ),
      );
    if (advertised) fetch.mockResolvedValueOnce(Response.json(["approved"]));
    fetch.mockResolvedValueOnce(new Response(null, { status: 204 }));
    const client = await new RustHttpClient("", fetch).attach("vault");
    const host = await preparePluginCredentials(client);
    try {
      if (advertised) {
        expect(host?.allows("approved")).toBe(true);
        expect(host?.allows("unapproved")).toBe(false);
        expect(fetch.mock.calls[1][0]).toBe(
          "/api/v3/clients/client/plugin-credentials",
        );
      } else {
        expect(host).toBeUndefined();
        expect(fetch).toHaveBeenCalledTimes(1);
      }
    } finally {
      await client.close();
    }
    if (advertised) expect(host?.allows("approved")).toBe(false);
  },
);
