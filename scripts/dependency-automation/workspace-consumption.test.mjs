import test from "node:test";
import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
  readFileSync,
} from "node:fs";
import { join } from "node:path";
import { checkWorkspaceIntake } from "./check-workspace-intake.mjs";
import {
  consumptionFixture,
  notePath,
  noteBytes,
} from "./consumption-fixture.mjs";
import { verifyWorkspaceNotes } from "./workspace-note.mjs";

for (const hasJj of [false, true])
  test(`actual Changesets consumption accepts unchanged dependency facts via ${hasJj ? "JJ" : "Git"} parser`, () => {
    const f = consumptionFixture({ hasJj });
    try {
      f.consumeWithActualCli();
      assert.equal(existsSync(join(f.root, notePath)), false);
      for (const [path, bytes] of Object.entries(f.data.before))
        assert.equal(readFileSync(join(f.root, path), "utf8"), bytes);
      assert.ok(f.sourceReader(f.root).changedFiles().includes(notePath));
      const facts = checkWorkspaceIntake(f.root, f.sourceReader);
      assert.equal(facts.repository, "lapismd/lapis-server");
    } finally {
      rmSync(f.root, { recursive: true, force: true });
    }
  });
test("genuine published dependency patch still requires bytes for a changed managed note", () => {
  const f = consumptionFixture({ changed: true });
  try {
    assert.throws(
      () => checkWorkspaceIntake(f.root, f.sourceReader),
      /managed dependency note bytes/,
    );
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});
test("same real dependency patch with actual empty note remains valid", () => {
  const f = consumptionFixture({ changed: true });
  try {
    f.present();
    assert.equal(
      checkWorkspaceIntake(f.root, f.sourceReader).repository,
      "lapismd/lapis-server",
    );
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});
for (const [name, prepare] of [
  ["directory", (f) => mkdirSync(join(f.root, notePath))],
  [
    "live internal symlink",
    (f) => {
      f.write("live.md", noteBytes);
      symlinkSync(join(f.root, "live.md"), join(f.root, notePath));
    },
  ],
  [
    "dangling symlink",
    (f) => symlinkSync(join(f.root, "absent.md"), join(f.root, notePath)),
  ],
  [
    "external symlink",
    (f) => {
      const target = join(f.root, "..", "outside-" + f.root.split("/").at(-1));
      writeFileSync(target, noteBytes);
      symlinkSync(target, join(f.root, notePath));
      return () => rmSync(target);
    },
  ],
  [
    "oversized regular file",
    (f) => f.present("---\n---\n\n" + "x".repeat(8192)),
  ],
  ["malformed live file", (f) => f.present("not frontmatter")],
])
  test(`consumption never hides current unsafe note ${name}`, () => {
    const f = consumptionFixture();
    let cleanup;
    try {
      cleanup = prepare(f);
      assert.throws(() => checkWorkspaceIntake(f.root, f.sourceReader));
    } finally {
      cleanup?.();
      rmSync(f.root, { recursive: true, force: true });
    }
  });
for (const [name, bytes] of [
  ["malformed", "not frontmatter"],
  ["runtime intent", '---\n"@lapismd/lapis-server": patch\n---\n\nBody\n'],
  ["null map", "---\nnull\n---\n\nBody\n"],
])
  test(`actual dependency intake rejects ${name} note`, () => {
    const f = consumptionFixture({ changed: true });
    try {
      f.present(bytes);
      assert.throws(() => checkWorkspaceIntake(f.root, f.sourceReader));
    } finally {
      rmSync(f.root, { recursive: true, force: true });
    }
  });
test("arbitrary external changed path is not read or admitted by dependency intake", () => {
  const f = consumptionFixture({ changed: true });
  try {
    f.present();
    f.externalChangedPath("../external.md");
    assert.throws(() => checkWorkspaceIntake(f.root, f.sourceReader), /scope/);
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});
test("public runtime release-map requirement remains strict independently of private Server policy", () => {
  assert.throws(
    () => verifyWorkspaceNotes([], {}, { "@lapismd/public-owner": "patch" }),
    /requires its Changeset/,
  );
});
