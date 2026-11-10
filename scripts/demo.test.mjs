import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  copyFileSync,
  writeFileSync,
  readFileSync,
  existsSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "lapis-demo-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, "scripts"));
  mkdirSync(join(root, "bin"));
  copyFileSync(
    new URL("demo.sh", import.meta.url),
    join(root, "scripts/demo.sh"),
  );
  copyFileSync(
    new URL("../.env.example", import.meta.url),
    join(root, ".env.example"),
  );
  const log = join(root, "docker.log");
  writeFileSync(
    join(root, "bin/docker"),
    '#!/bin/sh\nprintf "%s\\n" "$*" >> "$DEMO_TEST_LOG"\ncase "$*" in *" info "*) printf "%s\\n" "${DEMO_TEST_MEMORY:-6442450944}";; *" compose up "*) exit "${DEMO_TEST_UP_EXIT:-0}";; esac\n',
    { mode: 0o755 },
  );
  return {
    root,
    log,
    run(args = [], extra = {}) {
      return spawnSync("sh", [join(root, "scripts/demo.sh"), ...args], {
        cwd: tmpdir(),
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${join(root, "bin")}:${process.env.PATH}`,
          DEMO_TEST_LOG: log,
          LAPIS_DEMO_DOCKER_CONTEXT: "default",
          ...extra,
        },
      });
    },
  };
}

test("prepare creates defaults from any cwd, preserves existing configuration and password", (t) => {
  const f = fixture(t);
  assert.equal(f.run(["prepare"]).status, 0);
  const password = readFileSync(
    join(f.root, ".local/demo/bootstrap-password"),
    "utf8",
  );
  assert.match(password, /^[a-f0-9]{48}\n$/);
  assert.deepEqual(
    JSON.parse(readFileSync(join(f.root, ".local/demo/config.json"))),
    { vaults: [] },
  );
  writeFileSync(join(f.root, ".env"), "# operator settings\n");
  writeFileSync(
    join(f.root, ".local/demo/config.json"),
    '{"vaults":["retained"]}',
  );
  assert.equal(f.run(["prepare"]).status, 0);
  assert.equal(
    readFileSync(join(f.root, ".env"), "utf8"),
    "# operator settings\n",
  );
  assert.equal(
    readFileSync(join(f.root, ".local/demo/config.json"), "utf8"),
    '{"vaults":["retained"]}',
  );
  assert.equal(
    readFileSync(join(f.root, ".local/demo/bootstrap-password"), "utf8"),
    password,
  );
  assert.doesNotMatch(readFileSync(f.log, "utf8"), /compose up/);
});

test("startup builds and waits using the explicit default context", (t) => {
  const f = fixture(t);
  assert.equal(f.run().status, 0);
  const lines = readFileSync(f.log, "utf8").trim().split("\n");
  assert.ok(lines.every((line) => line.startsWith("--context default ")));
  assert.ok(
    lines.includes(
      "--context default build --file crates/lapis-server/Dockerfile --target web .",
    ),
  );
  assert.ok(
    lines.includes(
      "--context default build --file crates/lapis-server/Dockerfile --target native .",
    ),
  );
  assert.ok(
    lines.findIndex((line) => line.includes("--target web")) <
      lines.findIndex((line) => line.includes("--target native")),
  );
  assert.ok(
    lines.includes(
      "--context default compose up --build --detach --wait --wait-timeout 180",
    ),
  );
});

test("an explicitly selected local context is passed to every Docker call", (t) => {
  const f = fixture(t);
  assert.equal(
    f.run([], { LAPIS_DEMO_DOCKER_CONTEXT: "desktop-linux" }).status,
    0,
  );
  assert.ok(
    readFileSync(f.log, "utf8")
      .trim()
      .split("\n")
      .every((line) => line.startsWith("--context desktop-linux ")),
  );
});

test("failed startup returns failure without announcing readiness", (t) => {
  const f = fixture(t);
  const result = f.run([], { DEMO_TEST_UP_EXIT: "9" });
  assert.equal(result.status, 9);
  assert.doesNotMatch(result.stdout, /Demo ready/);
});

test("invalid action does not create configuration", (t) => {
  const f = fixture(t);
  assert.equal(f.run(["invalid"]).status, 2);
  assert.equal(existsSync(join(f.root, ".env")), false);
});

test("low Docker memory fails before building or changing configuration", (t) => {
  const f = fixture(t);
  const result = f.run([], { DEMO_TEST_MEMORY: "3052138496" });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /at least 6 GiB/);
  assert.doesNotMatch(readFileSync(f.log, "utf8"), /--target|compose up/);
  assert.equal(existsSync(join(f.root, ".env")), false);
});

test("published image mode pulls and starts without building or requiring build memory", (t) => {
  const f = fixture(t);
  const result = f.run(["image", "ghcr.io/lapismd/lapis-server:45f4c1b"], {
    DEMO_TEST_MEMORY: "3052138496",
  });
  assert.equal(result.status, 0);
  const log = readFileSync(f.log, "utf8");
  assert.match(log, /--context default compose pull/);
  assert.match(log, /compose up --no-build --detach --wait --wait-timeout 180/);
  assert.doesNotMatch(log, /--target|up --build/);
});

test("published image mode requires an explicit image", (t) => {
  const f = fixture(t);
  const result = f.run(["image"]);
  assert.equal(result.status, 2);
  assert.equal(existsSync(join(f.root, ".env")), false);
});
