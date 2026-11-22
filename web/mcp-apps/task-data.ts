import type {
  ApplicationTaskLine,
  TaskTrackingCompletionPrompt,
} from "@lapis-notes/lapis-plugin-tasks";
import type { ComponentProps } from "svelte";
import { z } from "zod";

export const listInput = z.object({
  vault_id: z.uuid(),
  view: z.string().optional(),
  today: z.string().optional(),
  limit: z.number().int().min(1).max(100).optional(),
});

const status = z.enum(["open", "completed", "canceled"]);
const priority = z.enum(["low", "medium", "high"]);
const row = z.object({
  kind: z.string(),
  documentId: z.string(),
  documentPath: z.string(),
  title: z.string(),
  status,
  deadline: z.string().nullish(),
  priority: priority.nullish(),
});
export const listResult = z.object({
  items: z.array(row).max(100),
  truncated: z.boolean(),
});

const plan = z.object({
  date: z.string(),
  at: z.enum(["anytime", "morning", "afternoon", "evening", "time"]),
  time: z.string().nullish(),
  duration: z.string().nullish(),
  durationMinutes: z.number().nullish(),
});
const tracking = z.discriminatedUnion("type", [
  z.object({ type: z.literal("boolean") }),
  z.object({
    type: z.literal("count"),
    unit: z.string(),
    target: z.number().optional(),
  }),
  z.object({
    type: z.literal("number"),
    unit: z.string(),
    target: z.number().optional(),
  }),
  z.object({ type: z.literal("duration"), target: z.string().optional() }),
]);
export const savedTask = z.object({
  snapshot: z.object({ path: z.string(), version: z.string() }),
  task: z.object({
    id: z.string(),
    path: z.string(),
    title: z.string(),
    kind: z.literal("task"),
    tags: z.array(z.string()),
    body: z.string(),
    created: z.string().optional(),
    updated: z.string().optional(),
    task: z.object({
      status,
      inbox: z.boolean().optional(),
      deadline: z.string().nullish(),
      plan: plan.nullish(),
      tracking: tracking.nullish(),
    }),
  }),
});

export const completionResult = z.object({
  operation: z
    .object({
      request_id: z.uuid(),
      path: z.string(),
      disposition: z.enum(["completed", "retryable", "divergent"]),
    })
    .nullable(),
});

export const writeResult = z.object({ kind: z.literal("snapshot") });
export const allowedTools = z.array(z.string());
export type ListedTask = z.infer<typeof row>;
export type SavedTask = z.infer<typeof savedTask>;

export function taskLine(
  task: ListedTask,
): ComponentProps<typeof ApplicationTaskLine>["task"] {
  return {
    id: task.documentId,
    body: task.title,
    done: task.status === "completed",
    status: task.status,
    dueAt: task.deadline ?? undefined,
    priority: task.priority ?? undefined,
  };
}

export function promptTask(
  saved: SavedTask,
): ComponentProps<typeof TaskTrackingCompletionPrompt>["task"] {
  const task = saved.task;
  return {
    id: task.id,
    title: task.title,
    status: task.task.status,
    tracking: task.task.tracking ?? undefined,
    sourcePath: task.path,
    notes: task.body,
    tags: task.tags,
    areas: [],
    createdAt: task.created ?? "",
    updatedAt: task.updated ?? "",
  };
}

export function localDate(at: Date): string {
  return [at.getFullYear(), at.getMonth() + 1, at.getDate()]
    .map((part) => String(part).padStart(2, "0"))
    .join("-");
}

// getRandomValues also works in opaque-origin iframes without a secure context.
export function invocationId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 15) | 64;
  bytes[8] = (bytes[8] & 63) | 128;
  const hex = Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    hex.slice(12, 16),
    hex.slice(16, 20),
    hex.slice(20),
  ].join("-");
}
