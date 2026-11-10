import { execFileSync, spawn } from "node:child_process";
import { cp, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";

const context = process.env.LAPIS_DOCKER_CONTEXT ?? "default";
const docker = (...args) =>
  execFileSync("docker", ["--context", context, ...args], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
const repository = resolve(import.meta.dirname, "..");
const root = await mkdtemp(join(repository, ".local/startup-budget-"));
const name = `lapis-startup-budget-${process.pid}`;
const image = `lapis-startup-budget:${process.pid}`;
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
await cp(join(repository, "web/dist"), join(root, "static"), {
  recursive: true,
});
await writeFile(join(root, "password"), password);
await writeFile(
  join(root, "config.json"),
  JSON.stringify({
    vaults: [
      {
        id: "3a65a94f-b875-4143-b1f5-a6a32d861092",
        name: "Budget vault",
        root: "/data/vault",
      },
    ],
  }),
);
await writeFile(
  join(root, "Dockerfile"),
  `FROM ghcr.io/lapismd/lapis-server:c0f4aae AS backend
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends libgcc-s1 ca-certificates && rm -rf /var/lib/apt/lists/* && groupadd -g 1993 lapis && useradd -u 1993 -g 1993 -d /data lapis && mkdir -p /data && chown 1993:1993 /data
COPY --from=backend /app/lapis-server /app/lapis-server
COPY static/ /app/static/
COPY --chown=1993:1993 vault/ /data/vault/
COPY password /run/secrets/password
COPY config.json /run/lapis/config.json
ENV HOME=/data LAPIS_SERVER_DATA_ROOT=/data/profile LAPIS_SERVER_STATIC_ROOT=/app/static LAPIS_SERVER_INSECURE_COOKIE=1 LAPIS_SERVER_BOOTSTRAP_USERNAME=owner LAPIS_SERVER_BOOTSTRAP_PASSWORD_FILE=/run/secrets/password
USER 1993:1993
ENTRYPOINT ["/app/lapis-server"]
CMD ["/run/lapis/config.json", "0.0.0.0:8787"]
`,
);
let tunnel, browser, page;
let tunnelError = "";
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
let overflow;
const exceeded = new Promise((_, reject) => {
  overflow = reject;
});
exceeded.catch(() => {});
try {
  console.log(
    `Building isolated request-budget fixture using Docker context ${context}`,
  );
  await writeFile(
    join(root, "build.log"),
    docker("build", "--load", "--tag", image, root),
  );
  const endpoint = JSON.parse(docker("context", "inspect", context))[0]
    .Endpoints.docker.Host;
  const remote = endpoint.startsWith("ssh://");
  docker(
    "run",
    "--detach",
    "--name",
    name,
    "--publish",
    remote ? "127.0.0.1::8787" : `127.0.0.1:${localPort}:8787`,
    "--env",
    `LAPIS_SERVER_ORIGIN=${origin}`,
    image,
  );
  if (remote) {
    const remotePort = docker("port", name, "8787/tcp")
      .trim()
      .split(":")
      .at(-1);
    const address = new URL(endpoint);
    tunnel = spawn(
      "ssh",
      [
        "-o",
        "ExitOnForwardFailure=yes",
        "-N",
        "-L",
        `${localPort}:127.0.0.1:${remotePort}`,
        `${address.username}@${address.hostname}`,
      ],
      { stdio: ["ignore", "ignore", "pipe"] },
    );
  }
  tunnel?.stderr.on("data", (bytes) => {
    tunnelError = (tunnelError + bytes).slice(-4096);
  });
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
    throw new Error(
      `Budget host did not start (${readiness}, tunnel ${tunnel?.exitCode} ${tunnelError}): ${docker("logs", name)}`,
    );
  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({ serviceWorkers: "block" });
  page.on("pageerror", (error) => consoleErrors.push(error.message));
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
      overflow(new Error("Startup request budget exceeded before readiness"));
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
  await writeFile(
    join(root, "failure.json"),
    JSON.stringify(
      {
        metrics,
        consoleErrors,
        text: await page
          ?.locator("body")
          .innerText()
          .catch(() => "unavailable"),
      },
      null,
      2,
    ),
  );
  console.error(
    JSON.stringify({ metrics, receipt: root, error: error.message }),
  );
  throw error;
} finally {
  await browser?.close();
  tunnel?.kill("SIGTERM");
  try {
    docker("rm", "--force", name);
  } catch {}
}
