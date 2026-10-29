import { z } from "zod/v3";

import type { AppDatabase } from "./app-database";

export const REMOTE_APP_DATABASE_MAX_ARGUMENTS = 4;
export const REMOTE_APP_DATABASE_MAX_MESSAGE_BYTES = 32 * 1024 * 1024;

export const REMOTE_APP_DATABASE_METHODS = [
  "migrate",
  "beginSearchIndexingBatch",
  "endSearchIndexingBatch",
  "configureSearchEmbeddingProvider",
  "getSearchEmbeddingProvider",
  "getSearchEmbeddingRuntimeStatus",
  "getSearchIndexStats",
  "getMeta",
  "setMeta",
  "scanMeta",
  "transactMeta",
  "getNotebookState",
  "setNotebookState",
  "deleteNotebookState",
  "loadMetadataSnapshot",
  "saveMetadataSnapshot",
  "getFileHistory",
  "storeFileHistoryRevision",
  "listNotifications",
  "upsertNotification",
  "markNotificationRead",
  "clearNotification",
  "clearAllNotifications",
  "getMemorySourceState",
  "listMemorySourceStates",
  "upsertMemorySourceState",
  "deleteMemorySourceState",
  "upsertMemoryCandidate",
  "queryMemoryCandidates",
  "deleteMemoryCandidatesByConversation",
  "recordMemoryRecallSignal",
  "listMemoryRecallSignals",
  "claimMemoryJob",
  "updateMemoryJob",
  "listMemoryJobs",
  "clearMemoryDerivedState",
  "getChangeRevision",
  "upsertIndexedFile",
  "getIndexedFile",
  "listIndexedFileManifest",
  "queryIndexedMetadata",
  "queryIndexedMetadataPage",
  "queryMetadataFacets",
  "queryMetadataLinks",
  "deleteIndexedFile",
  "renameIndexedFile",
  "upsertSearchDocument",
  "deleteSearchDocument",
  "getSearchDocument",
  "listSearchDocumentManifest",
  "listSearchDocuments",
  "rebuildSearchIndex",
  "searchDocuments",
  "searchDocumentPaths",
  "upsertTaskProjection",
  "deleteTaskProjection",
  "queryTasks",
  "getTaskRow",
  "listChildLinks",
  "listTaskDescendants",
  "registerProjectionDefinition",
  "unregisterProjectionDefinition",
  "replaceProjectionSource",
  "markProjectionSourceError",
  "deleteProjectionSource",
  "queryProjection",
  "getProjectionRow",
  "queryRelated",
] as const satisfies readonly (keyof AppDatabase)[];

export type RemoteAppDatabaseMethod =
  (typeof REMOTE_APP_DATABASE_METHODS)[number];

export type RemoteAppDatabaseArgs<Method extends RemoteAppDatabaseMethod> =
  Method extends RemoteAppDatabaseMethod
    ? AppDatabase[Method] extends (...args: infer Args) => unknown
      ? Args
      : never
    : never;

export type RemoteAppDatabaseResult<Method extends RemoteAppDatabaseMethod> =
  AppDatabase[Method] extends (...args: never[]) => infer Result
    ? Awaited<Result>
    : never;

export type RemoteAppDatabaseDomain =
  | "lifecycle"
  | "metadata"
  | "search"
  | "history"
  | "notifications"
  | "memory"
  | "tasks"
  | "projections";

export interface RemoteAppDatabaseContractEntry<
  Method extends RemoteAppDatabaseMethod,
> {
  readonly method: Method;
  readonly domain: RemoteAppDatabaseDomain;
  readonly input: z.ZodType<RemoteAppDatabaseArgs<Method>>;
  readonly output: z.ZodType<RemoteAppDatabaseResult<Method>>;
}

const remoteMethodSet = new Set<string>(REMOTE_APP_DATABASE_METHODS);

function jsonBytes(value: unknown): number {
  const json = JSON.stringify(value);
  return typeof json === "string"
    ? new TextEncoder().encode(json).byteLength
    : 0;
}

function isBoundedJson(value: unknown): boolean {
  try {
    return jsonBytes(value) <= REMOTE_APP_DATABASE_MAX_MESSAGE_BYTES;
  } catch {
    return false;
  }
}

function methodDomain(
  method: RemoteAppDatabaseMethod,
): RemoteAppDatabaseDomain {
  if (/memory/iu.test(method)) return "memory";
  if (/notification/iu.test(method)) return "notifications";
  if (/history/iu.test(method)) return "history";
  if (/task|childLinks/iu.test(method)) return "tasks";
  if (/projection|related/iu.test(method)) return "projections";
  if (/search/iu.test(method)) return "search";
  if (
    method === "migrate" ||
    method === "beginSearchIndexingBatch" ||
    method === "endSearchIndexingBatch" ||
    method === "getChangeRevision" ||
    method === "clearMemoryDerivedState"
  ) {
    return "lifecycle";
  }
  return "metadata";
}

function contractEntry<Method extends RemoteAppDatabaseMethod>(
  method: Method,
): RemoteAppDatabaseContractEntry<Method> {
  return {
    method,
    domain: methodDomain(method),
    input: z.custom<RemoteAppDatabaseArgs<Method>>(
      (value) =>
        Array.isArray(value) &&
        value.length <= REMOTE_APP_DATABASE_MAX_ARGUMENTS &&
        isBoundedJson(value),
      "Invalid or oversized AppDatabase arguments",
    ),
    output: z.custom<RemoteAppDatabaseResult<Method>>(
      (value) => value === undefined || isBoundedJson(value),
      "Invalid or oversized AppDatabase result",
    ),
  };
}

export const REMOTE_APP_DATABASE_CONTRACT = Object.fromEntries(
  REMOTE_APP_DATABASE_METHODS.map((method) => [method, contractEntry(method)]),
) as {
  readonly [Method in RemoteAppDatabaseMethod]: RemoteAppDatabaseContractEntry<Method>;
};

export function isRemoteAppDatabaseMethod(
  value: unknown,
): value is RemoteAppDatabaseMethod {
  return typeof value === "string" && remoteMethodSet.has(value);
}

export function parseRemoteAppDatabaseArgs<
  Method extends RemoteAppDatabaseMethod,
>(method: Method, value: unknown): RemoteAppDatabaseArgs<Method> {
  return REMOTE_APP_DATABASE_CONTRACT[method].input.parse(value);
}

export function parseRemoteAppDatabaseResult<
  Method extends RemoteAppDatabaseMethod,
>(method: Method, value: unknown): RemoteAppDatabaseResult<Method> {
  return REMOTE_APP_DATABASE_CONTRACT[method].output.parse(value);
}
