import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { checkWorkspaceIntake } from "./check-workspace-intake.mjs";
export { checkWorkspaceIntake };
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  checkWorkspaceIntake();
  console.log("Server workspace canonical intake matches actual source facts.");
}
