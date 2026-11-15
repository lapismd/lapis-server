import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import SyncAccountConsent from "./SyncAccountConsent.svelte";
import { RustHttpClient } from "./hosted/rust/client";

afterEach(cleanup);

test("retired consent screen cannot continue inspection after a late response", async () => {
  let resolve!: (value: Response) => void;
  const response = new Promise<Response>((done) => {
    resolve = done;
  });
  const fetch = vi.fn<typeof globalThis.fetch>(() => response);
  const view = render(SyncAccountConsent, {
    requestId: id,
    host: new RustHttpClient("", fetch),
  });
  await waitFor(() => expect(fetch).toHaveBeenCalledTimes(1));
  view.unmount();
  resolve(Response.json(info));
  await response;
  await new Promise((done) => setTimeout(done, 0));
  expect(fetch).toHaveBeenCalledTimes(1);
});
const id = "82791527-4c73-4820-84f4-5db3f7ee5400";
const clientId = "e09dd6f5-9bbc-4faa-aefc-b55fa4eef854";
const metadata = {
  id,
  client_id: clientId,
  client_name: "My Desktop",
  expires_at: Date.now() + 60_000,
  status: "pending",
};
const info = { protocol_major: 3, browser_auth: true };
const session = { username: "Owner", csrf_token: "owner-csrf" };
function fixture(decisionFails = false) {
  const fetch = vi.fn<typeof globalThis.fetch>(async (url, init) => {
    const path = String(url);
    if (path.endsWith("/system/info")) return Response.json(info);
    if (path.endsWith("/auth/current") || path.endsWith("/auth/login"))
      return Response.json(session);
    if (path.endsWith("/decision")) {
      if (decisionFails) throw new Error("Lost acknowledgement");
      return Response.json({
        approved: JSON.parse(String(init?.body)).approve,
      });
    }
    return Response.json(metadata);
  });
  return { fetch, host: new RustHttpClient("", fetch) };
}

test.each([true, false])(
  "owner explicitly decides %s with CSRF and no native secrets",
  async (approve) => {
    const { host, fetch } = fixture();
    const view = render(SyncAccountConsent, { requestId: id, host });
    await view.findByRole("button", { name: "Approve connection" });
    expect(view.getByText("My Desktop")).toBeTruthy();
    expect(
      fetch.mock.calls.filter(([, init]) => init?.method === "POST"),
    ).toHaveLength(0);
    await fireEvent.click(
      view.getByRole("button", {
        name: approve ? "Approve connection" : "Reject",
      }),
    );
    expect((await view.findByRole("status")).textContent).toContain(
      approve ? "approved" : "rejected",
    );
    const call = fetch.mock.calls.find(([url]) =>
      String(url).endsWith("/decision"),
    )!;
    expect(JSON.parse(String(call[1]?.body))).toEqual({ approve });
    expect(new Headers(call[1]?.headers).get("x-lapis-csrf")).toBe(
      "owner-csrf",
    );
    expect(
      fetch.mock.calls.some(([url]) => /redeem|discovery/u.test(String(url))),
    ).toBe(false);
  },
);

test("lost decision response requires inspection and cannot replay from the screen", async () => {
  const { host, fetch } = fixture(true);
  const view = render(SyncAccountConsent, { requestId: id, host });
  await fireEvent.click(
    await view.findByRole("button", { name: "Approve connection" }),
  );
  expect((await view.findByRole("alert")).textContent).toContain(
    "Reload this page to inspect",
  );
  expect(view.queryByRole("button", { name: "Approve connection" })).toBeNull();
  expect(
    fetch.mock.calls.filter(([url]) => String(url).endsWith("/decision")),
  ).toHaveLength(1);
});

test("invalid links make no network requests", async () => {
  const { host, fetch } = fixture();
  const view = render(SyncAccountConsent, { requestId: "../../foreign", host });
  expect((await view.findByRole("alert")).textContent).toContain("invalid");
  expect(fetch).not.toHaveBeenCalled();
});

test("sign-in retains the request and clears the password before review", async () => {
  const { fetch } = fixture();
  let loggedIn = false;
  const delegate = fetch.getMockImplementation()!;
  fetch.mockImplementation(async (url, init) => {
    if (String(url).endsWith("/auth/current") && !loggedIn)
      return Response.json({ code: "PERMISSION_DENIED" }, { status: 401 });
    if (String(url).endsWith("/auth/login")) loggedIn = true;
    return delegate(url, init);
  });
  const view = render(SyncAccountConsent, {
    requestId: id,
    host: new RustHttpClient("", fetch),
  });
  await view.findByLabelText("Username");
  await fireEvent.input(view.getByLabelText("Username"), {
    target: { value: "owner" },
  });
  await fireEvent.input(view.getByLabelText("Password"), {
    target: { value: "temporary-password" },
  });
  const password = view.getByLabelText("Password") as HTMLInputElement;
  await fireEvent.submit(password.closest("form")!);
  await view.findByRole("button", { name: "Approve connection" });
  expect(view.queryByLabelText("Password")).toBeNull();
  expect(
    fetch.mock.calls.filter(([url]) => String(url).endsWith("/decision")),
  ).toHaveLength(0);
});
