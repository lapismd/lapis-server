import { spawn } from "node:child_process";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";

const repository = resolve(import.meta.dirname, "..");
const root = await mkdtemp(join(repository, ".local/startup-budget-"));
const listener = createServer();
await new Promise((accept) => listener.listen(0, "127.0.0.1", accept));
const localPort = listener.address().port;
await new Promise((accept) => listener.close(accept));
const origin = `http://localhost:${localPort}`;
const password = "isolated-startup-budget-password";
const vault = join(root, "vault");
await mkdir(join(vault, ".lapis/plugins/budget-assets/assets"), {
  recursive: true,
});
await writeFile(join(vault, "Note.md"), "# Startup budget fixture\n");
await writeFile(
  join(vault, ".lapis/core-plugins.json"),
  JSON.stringify(["file-explorer", "markdown"]),
);
await writeFile(join(vault, ".lapis/community-plugins.json"), "[]");
await writeFile(
  join(vault, ".lapis/plugins/budget-assets/manifest.json"),
  JSON.stringify({
    id: "budget-assets",
    name: "Dormant asset fixture",
    version: "0.0.0",
    minAppVersion: "0.1.0",
    author: "Test",
  }),
);
for (let index = 0; index < 600; index++) {
  await writeFile(
    join(vault, `.lapis/plugins/budget-assets/assets/chunk-${index}.mjs`),
    `export const value = ${index};\n`,
  );
}
await writeFile(join(root, "password"), password);
await writeFile(
  join(root, "config.json"),
  JSON.stringify({
    vaults: [
      {
        id: "3a65a94f-b875-4143-b1f5-a6a32d861092",
        name: "Budget vault",
        root: vault,
      },
    ],
  }),
);
let service, browser, page;
let logs = "";
let readiness = "unreachable";
const metrics = {
  commands: 0,
  database: 0,
  kinds: {},
  databaseMethods: {},
  atReadiness: null,
  sampleMs: 20000,
  pluginAssets: 600,
};
const consoleErrors = [];
const networkErrors = [];
const retain = (items, value) => {
  if (items.length < 32) items.push(value);
};
let overflow;
const exceeded = new Promise((_, reject) => {
  overflow = reject;
});
exceeded.catch(() => {});
try {
  const binary = join(repository, ".local/rust-core-target/debug/lapis-server");
  console.log(`Qualifying checkout backend ${binary} with checkout web/dist`);
  const environment = {
    ...process.env,
    LAPIS_SERVER_ORIGIN: origin,
    LAPIS_SERVER_DATA_ROOT: join(root, "profile"),
    LAPIS_SERVER_STATIC_ROOT: join(repository, "web/dist"),
    LAPIS_SERVER_INSECURE_COOKIE: "1",
    LAPIS_SERVER_BOOTSTRAP_USERNAME: "owner",
    LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE: join(root, "password"),
    LAPIS_SERVER_NATIVE_HISTORY: "1",
  };
  delete environment.LAPIS_SERVER_TOKEN;
  service = spawn(
    binary,
    [join(root, "config.json"), `127.0.0.1:${localPort}`],
    {
      env: environment,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  const retainLog = (bytes) => {
    logs = (logs + String(bytes)).slice(-32768);
  };
  service.stdout.on("data", retainLog);
  service.stderr.on("data", retainLog);
  service.on("error", (error) => retainLog(error.message));
  let ready = false;
  for (let attempt = 0; attempt < 60; attempt++) {
    try {
      const response = await fetch(`${origin}/readyz`);
      readiness = String(response.status);
      if (response.ok) {
        ready = true;
        break;
      }
    } catch {}
    await new Promise((accept) => setTimeout(accept, 500));
  }
  if (!ready)
    throw new Error(`Budget host did not start (${readiness}): ${logs}`);
  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({ serviceWorkers: "block" });
  page.on("pageerror", (error) => retain(consoleErrors, error.message));
  page.on("console", (message) => {
    if (message.type() === "error")
      retain(consoleErrors, message.text().slice(0, 4096));
  });
  page.on("requestfailed", (request) =>
    retain(networkErrors, {
      path: new URL(request.url()).pathname,
      error: request.failure()?.errorText,
    }),
  );
  page.on("response", (response) => {
    if (response.status() >= 400)
      retain(networkErrors, {
        path: new URL(response.url()).pathname,
        status: response.status(),
      });
  });
  page.on("request", (request) => {
    if (request.method() !== "POST") return;
    if (request.url().includes("/commands")) {
      metrics.commands++;
      let body;
      try {
        body = request.postDataJSON();
      } catch {
        body = null;
      }
      const kind = body?.command?.kind ?? body?.kind ?? "unknown";
      metrics.kinds[kind] = (metrics.kinds[kind] ?? 0) + 1;
    }
    if (request.url().includes("/database")) {
      metrics.database++;
      let method = "unknown";
      try {
        method = request.postDataJSON()?.method ?? method;
      } catch {}
      metrics.databaseMethods[method] =
        (metrics.databaseMethods[method] ?? 0) + 1;
    }
    if (metrics.commands >= 200 || metrics.database >= 100)
      overflow(new Error("Startup request budget exceeded"));
  });
  await page.goto(origin);
  await page.getByLabel("Username").fill("owner");
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page
    .getByRole("button", {
      name: "Open Budget vault owner · server-hosted",
      exact: true,
    })
    .click();
  await Promise.race([
    page.getByText("Server DB", { exact: true }).waitFor({ timeout: 60000 }),
    exceeded,
  ]);
  metrics.atReadiness = {
    commands: metrics.commands,
    database: metrics.database,
  };
  await Promise.race([page.waitForTimeout(metrics.sampleMs), exceeded]);
  await writeFile(join(root, "metrics.json"), JSON.stringify(metrics, null, 2));
  console.log(JSON.stringify(metrics));
  if (metrics.commands >= 200 || metrics.database >= 100)
    throw new Error(
      `Startup request budget exceeded; receipt: ${root}/metrics.json`,
    );
} catch (error) {
  const failure = {
    metrics,
    consoleErrors,
    networkErrors,
    error: error.message,
    url: page?.url(),
    text: await page
      ?.locator("body")
      .innerText()
      .then((text) => text.slice(0, 8192))
      .catch(() => "unavailable"),
  };
  await writeFile(join(root, "failure.json"), JSON.stringify(failure, null, 2));
  console.error(JSON.stringify({ ...failure, receipt: root }));
  throw error;
} finally {
  await browser?.close();
  if (service && service.exitCode === null) {
    const stopped = new Promise((resolve) => service.once("exit", resolve));
    service.kill("SIGTERM");
    const timeout = setTimeout(() => service.kill("SIGKILL"), 5000);
    await stopped;
    clearTimeout(timeout);
  }
  await writeFile(join(root, "server.log"), logs);
}
