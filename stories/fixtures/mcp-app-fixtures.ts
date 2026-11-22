import type { CallToolResult } from "@modelcontextprotocol/client";

export type McpAppKind = "notes" | "tasks";

const vault = "3a65a94f-b875-4143-b1f5-a6a32d861092";
const available = [
  "get",
  "plan",
  "deadline",
  "inbox",
  "complete",
  "inspect",
].map((name) => `tasks_${name}`);

function reply(value: object, meta?: Record<string, unknown>): CallToolResult {
  const body = { outcome: { status: "ok", value } };
  return {
    content: [{ type: "text", text: JSON.stringify(body) }],
    structuredContent: body,
    ...(meta ? { _meta: meta } : {}),
  };
}

function fail(code = "INVALID_REQUEST"): CallToolResult {
  const body = {
    outcome: { status: "error", error: { code, effect: "not_started" } },
  };
  return {
    content: [{ type: "text", text: code }],
    structuredContent: body,
    isError: true,
  };
}

export function noteFixture() {
  const args = { vault_id: vault, path: "Projects/Team Offsite.md" };
  let revision = 1;
  function note() {
    return reply({
      kind: "note",
      path: args.path,
      version: String(revision),
      text: `# Team offsite

The design and engineering teams will meet on **Thursday, 10 December** at the Shoreditch studio.

## Morning agenda

| Time | Session |
| --- | --- |
| 09:30 | Coffee and arrivals |
| 10:00 | Review the prototype |
| 11:15 | Breakout discussions |

Bring one question you want the team to answer before the next release.${
        revision > 1 ? "\n\n> Latest update: The studio opens at 09:15." : ""
      }`,
    });
  }
  return {
    args,
    result: note(),
    call(name: string, input: Record<string, unknown>) {
      if (
        name !== "read_note" ||
        input.vault_id !== vault ||
        input.path !== args.path
      )
        return fail();
      revision++;
      return note();
    },
  };
}

type Plan = { date: string; at: string; time?: string; duration?: string };
type Task = {
  id: string;
  title: string;
  version: number;
  status: "open" | "completed";
  plan: Plan | null;
  deadline: string | null;
  inbox: boolean;
  tracking?: { type: "duration"; target: string };
  checklist?: { text: string; checked: boolean }[];
};

export function taskFixture() {
  const localDate = (date: Date) =>
    new Intl.DateTimeFormat("sv-SE", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).format(date);
  const now = new Date();
  const today = localDate(now);
  now.setDate(now.getDate() - 1);
  const yesterday = localDate(now);
  const tasks: Task[] = [
    {
      id: "agenda",
      title: "Review the offsite agenda",
      version: 1,
      status: "open",
      plan: { date: yesterday, at: "anytime" },
      deadline: null,
      inbox: false,
    },
    {
      id: "prototype",
      title: "Practice the prototype walkthrough",
      version: 1,
      status: "open",
      plan: { date: today, at: "anytime" },
      deadline: today,
      checklist: [
        { text: "Review the demo outline", checked: true },
        { text: "Rehearse the walkthrough", checked: false },
        { text: "Check the meeting room setup", checked: false },
      ],
      inbox: false,
      tracking: { type: "duration", target: "30m" },
    },
    {
      id: "studio",
      title: "Confirm access to the studio",
      version: 1,
      status: "open",
      plan: { date: today, at: "morning" },
      deadline: today,
      inbox: true,
    },
  ];
  const receipts = new Map<unknown, object>();
  const pathFor = (task: Task) => `Tasks/${task.id}.md`;
  function list() {
    return reply(
      {
        truncated: false,
        items: tasks.map((task) => ({
          kind: "task",
          documentId: task.id,
          documentPath: pathFor(task),
          title: task.title,
          status: task.status,
          deadline: task.deadline,
          planDate: task.plan?.date,
          planKind: task.plan?.at,
          planTime: task.plan?.time,
          checklistTotal: task.checklist?.length ?? 0,
          checklistCompleted:
            task.checklist?.filter((item) => item.checked).length ?? 0,
        })),
      },
      { "io.lapismd/availableTools": available },
    );
  }
  return {
    args: { vault_id: vault, limit: 50 },
    result: list(),
    call(name: string, input: Record<string, unknown>) {
      if (input.vault_id !== vault) return fail();
      if (name === "tasks_list") return list();
      if (name === "tasks_inspect")
        return reply({ operation: receipts.get(input.request_id) ?? null });
      const task = tasks.find((entry) => pathFor(entry) === input.path);
      if (!task) return fail("NOT_FOUND");
      if (name === "tasks_get") {
        return reply({
          snapshot: { path: pathFor(task), version: String(task.version) },
          task: {
            id: task.id,
            path: pathFor(task),
            title: task.title,
            kind: "task",
            tags: [],
            body:
              task.id === "prototype" ? "Rehearse the demo in the studio." : "",
            checklist:
              task.checklist?.map((item, position) => ({
                ...item,
                position,
                sourceStart: 0,
                sourceEnd: 0,
                source: `- [${item.checked ? "x" : " "}] ${item.text}`,
              })) ?? [],
            task,
          },
        });
      }
      if (input.expected_version !== String(task.version))
        return fail("CONFLICT");
      switch (name) {
        case "tasks_plan":
          task.plan = input.plan as Plan | null;
          break;
        case "tasks_deadline":
          task.deadline = input.deadline as string | null;
          break;
        case "tasks_inbox":
          task.inbox = input.inbox as boolean;
          break;
        case "tasks_complete": {
          if (task.tracking && typeof input.trackedValue !== "number")
            return fail();
          task.status = "completed";
          task.version++;
          const operation = {
            request_id: input.invocation_id,
            path: input.path,
            disposition: "completed",
          };
          receipts.set(input.invocation_id, operation);
          return reply({ operation });
        }
        default:
          return fail();
      }
      task.version++;
      return reply({ kind: "snapshot" });
    },
  };
}
