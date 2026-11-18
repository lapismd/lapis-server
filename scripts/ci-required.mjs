const plan = JSON.parse(process.env.LAPIS_CI_PLAN ?? "null");
const jobs = JSON.parse(process.env.LAPIS_CI_JOBS ?? "null");
if (!plan || !jobs || jobs.plan?.result !== "success")
  throw Error("Missing successful CI plan");
const expected = {
  governance: true,
  tooling: plan.tooling,
  configure: plan.configure,
  rust: plan.rustPackages.length > 0,
  "mod-loader": plan.modLoader,
  renderer: plan.renderer,
  "image-tooling": plan.imageTooling,
};
for (const [name, selected] of Object.entries(expected)) {
  const result = jobs[name]?.result;
  if (
    selected ? result !== "success" : !["success", "skipped"].includes(result)
  )
    throw Error(
      `${name} finished ${result ?? "missing"}; selected=${selected}`,
    );
}
console.log("Every selected Server CI gate passed.");
