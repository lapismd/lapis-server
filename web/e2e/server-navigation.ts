import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import type { Page, Request, TestInfo } from "@playwright/test";

export interface ServerNavigationRetry {
  failedRequests: string[];
  firstFailure: string;
}

export async function recordServerNavigationRetry(
  testInfo: Pick<TestInfo, "annotations" | "attach" | "outputPath">,
  retry: ServerNavigationRetry,
): Promise<void> {
  console.warn(
    "Retrying initial server navigation once after Chromium ERR_NETWORK_CHANGED",
  );
  testInfo.annotations.push({
    type: "network-change-retry",
    description:
      "One initial navigation retry after Chromium aborted a document or script request",
  });
  const report = testInfo.outputPath("server-navigation-network-change.json");
  await mkdir(dirname(report), { recursive: true });
  await writeFile(report, JSON.stringify(retry, null, 2), "utf8");
  await testInfo.attach("server-navigation-network-change", {
    path: report,
    contentType: "application/json",
  });
}

export async function navigateServerPage(
  page: Page,
  url: string,
  assertReady: () => Promise<void>,
  onRetry?: (retry: ServerNavigationRetry) => Promise<void>,
): Promise<number> {
  const origin = new URL(url).origin;
  for (let attempt = 1; attempt <= 2; attempt++) {
    const failedRequests: string[] = [];
    const observeFailure = (request: Request) => {
      if (
        request.failure()?.errorText === "net::ERR_NETWORK_CHANGED" &&
        ["document", "script"].includes(request.resourceType()) &&
        new URL(request.url()).origin === origin
      ) {
        failedRequests.push(request.url());
      }
    };
    page.on("requestfailed", observeFailure);
    let navigating = true;
    try {
      await page.goto(url);
      navigating = false;
      await assertReady();
      return attempt;
    } catch (error) {
      const navigationChanged =
        navigating && /\bnet::ERR_NETWORK_CHANGED\b/u.test(String(error));
      if (attempt === 2 || (!navigationChanged && !failedRequests.length))
        throw error;
      await onRetry?.({
        failedRequests: [...failedRequests],
        firstFailure: String(error),
      });
    } finally {
      page.off("requestfailed", observeFailure);
    }
  }
  throw new Error("Server startup retry exhausted");
}
