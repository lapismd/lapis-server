#!/usr/bin/env node
// Adapted from jj-store scripts/test-vault-https.mjs at 086def18 (Apache-2.0).
// Trust installation is confined to the disposable validation container.
import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  copyFileSync,
  rmSync,
  realpathSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import { httpsFixture } from "./repository-https-endpoint.mjs";
if (
  process.platform !== "linux" ||
  process.getuid() !== 0 ||
  process.env.LAPIS_SERVER_DISPOSABLE_HTTPS !== "1"
)
  throw Error("HTTPS fixture requires the disposable Linux validation worker");
if (process.argv.length > 2) throw Error("HTTPS fixture takes no arguments");
const base = realpathSync(mkdtempSync(join(homedir(), ".jj-vault-https-")));
const certificate =
  "/usr/local/share/ca-certificates/lapis-disposable-" + randomUUID() + ".crt";
const environment = {
  ...process.env,
  HOME: join(base, "home"),
  JJ_CONFIG: "/dev/null",
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
};
const run = (bin, args) =>
  execFileSync(bin, args, {
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });
let server;
try {
  mkdirSync(environment.HOME, { mode: 0o700 });
  mkdirSync(join(base, "exports"));
  for (const name of ["token", "anonymous"])
    run("git", [
      "init",
      "--quiet",
      "--bare",
      "--initial-branch=main",
      join(base, "exports", name + ".git"),
    ]);
  run("openssl", [
    "req",
    "-x509",
    "-newkey",
    "rsa:2048",
    "-sha256",
    "-nodes",
    "-days",
    "1",
    "-subj",
    "/CN=Lapis disposable TLS fixture",
    "-addext",
    "basicConstraints=critical,CA:TRUE",
    "-keyout",
    join(base, "ca.key"),
    "-out",
    join(base, "ca.crt"),
  ]);
  run("openssl", [
    "req",
    "-newkey",
    "rsa:2048",
    "-nodes",
    "-subj",
    "/CN=localhost",
    "-keyout",
    join(base, "server.key"),
    "-out",
    join(base, "server.csr"),
  ]);
  writeFileSync(
    join(base, "extensions"),
    "subjectAltName=IP:127.0.0.1,DNS:localhost\nextendedKeyUsage=serverAuth\n",
  );
  run("openssl", [
    "x509",
    "-req",
    "-in",
    join(base, "server.csr"),
    "-CA",
    join(base, "ca.crt"),
    "-CAkey",
    join(base, "ca.key"),
    "-CAcreateserial",
    "-days",
    "1",
    "-sha256",
    "-extfile",
    join(base, "extensions"),
    "-out",
    join(base, "server.crt"),
  ]);
  copyFileSync(join(base, "ca.crt"), certificate);
  run("update-ca-certificates", []);
  writeFileSync(join(base, "active-token"), "first-synthetic-token", {
    mode: 0o600,
  });
  server = await httpsFixture(base, environment);
  const { qualifyColdHttpsHost } = await import(
    "./repository-https-acceptance.mjs"
  );
  await qualifyColdHttpsHost(base, server.endpoint, environment);
  console.log(
    "Cold compiled Server HTTPS acceptance passed; disposable state retired.",
  );
} finally {
  await server?.close();
  rmSync(certificate, { force: true });
  run("update-ca-certificates", []);
  rmSync(base, { recursive: true, force: true });
}
