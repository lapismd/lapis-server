<script lang="ts">
  import {
    ApplicationTaskLine,
    TaskTrackingCompletionPrompt,
  } from "@lapis-notes/lapis-plugin-tasks";
  import { Button } from "@lapismd/design-core/shadcn/button";
  import { onMount } from "svelte";
  import { z } from "zod";
  import {
    connectView,
    ToolFailure,
    toolValue,
    type ToolBridge,
  } from "./bridge";
  import {
    allowedTools,
    completionResult,
    invocationId,
    listInput,
    listResult,
    localDate,
    promptTask,
    savedTask,
    taskLine,
    writeResult,
    type SavedTask,
  } from "./task-data";
  import TaskFields from "./TaskFields.svelte";

  let bridge: ToolBridge;
  let listArgs = $state.raw<z.infer<typeof listInput>>();
  let list = $state.raw<z.infer<typeof listResult>>();
  let selected = $state.raw<SavedTask>();
  let selectedPath = $state("");
  let tools = $state.raw<string[]>([]);
  let canCall = $state(false);
  let busy = $state(true);
  let prompt = $state(false);
  let error = $state("");
  let notice = $state("");
  let stale = $state(false);
  let uncertain = $state<{ id: string; path: string }>();

  onMount(() => {
    bridge = connectView("Lapis Tasks", {
      input(value) {
        listArgs = listInput.safeParse(value).data;
      },
      result(result) {
        try {
          list = toolValue(result, listResult);
          tools =
            allowedTools.safeParse(result._meta?.["io.lapismd/availableTools"])
              .data ?? [];
        } catch {
          error = "The tasks could not be read.";
        }
        busy = false;
      },
      ready(available) {
        canCall = available;
      },
      failed(message) {
        error = message;
        busy = false;
      },
    });
    return () => bridge.stop();
  });

  function canUse(name: string) {
    return canCall && tools.includes(name);
  }

  async function loadTask(vault: string, path: string) {
    const next = toolValue(
      await bridge.call("tasks_get", { vault_id: vault, path }),
      savedTask,
    );
    if (next.task.path !== path || next.snapshot.path !== path) {
      throw new Error("Unexpected task identity");
    }
    selected = next;
    stale = false;
  }

  async function select(path: string) {
    if (busy || uncertain || !listArgs || !canCall) return;
    selectedPath = path;
    selected = undefined;
    busy = true;
    error = "";
    notice = "";
    try {
      await loadTask(listArgs.vault_id, path);
    } catch {
      error = "The saved task could not be read.";
    } finally {
      busy = false;
    }
  }

  async function loadList() {
    if (!listArgs) return;
    const result = await bridge.call("tasks_list", listArgs);
    list = toolValue(result, listResult);
    tools =
      allowedTools.safeParse(result._meta?.["io.lapismd/availableTools"])
        .data ?? [];
  }

  async function refresh() {
    if (busy || !listArgs) return;
    busy = true;
    error = "";
    try {
      await loadList();
    } catch {
      error = "The tasks could not be refreshed.";
    } finally {
      busy = false;
    }
  }

  async function afterWrite(vault: string, path: string, message: string) {
    notice = message;
    try {
      await loadTask(vault, path);
      await loadList();
    } catch {
      error = "Some task data could not be refreshed.";
    }
  }

  async function write(name: string, fields: Record<string, unknown>) {
    const current = selected;
    if (!current || !listArgs || busy || stale || uncertain || !canUse(name))
      return;
    const vault = listArgs.vault_id;
    const path = current.snapshot.path;
    const id = invocationId();
    const at = new Date();
    const completing = name === "tasks_complete";
    busy = true;
    prompt = false;
    notice = "";
    error = "";
    try {
      const result = await bridge.call(name, {
        ...fields,
        vault_id: vault,
        path,
        expected_version: current.snapshot.version,
        invocation_id: id,
        at: at.toISOString(),
        ...(completing ? { localDate: localDate(at) } : {}),
      });
      if (completing) {
        const receipt = toolValue(result, completionResult).operation;
        if (
          receipt?.request_id !== id ||
          receipt.path !== path ||
          receipt.disposition !== "completed"
        ) {
          throw new Error("Completion requires journal confirmation");
        }
      } else {
        toolValue(result, writeResult);
      }
      stale = true;
      await afterWrite(
        vault,
        path,
        completing ? "Task completed." : "Task updated.",
      );
    } catch (cause) {
      stale = true;
      if (cause instanceof ToolFailure && cause.effect === "not_started") {
        error =
          cause.code === "CONFLICT"
            ? "The task changed or is open for editing. Reload the saved task before trying again."
            : "The server refused the task change. Reload the saved task before trying again.";
      } else if (completing) {
        uncertain = { id, path };
        error =
          "Completion could not be confirmed. Check its status before making another change.";
      } else {
        error = `The change could not be confirmed. Reload the saved task. Request ID: ${id}`;
      }
    } finally {
      busy = false;
    }
  }

  function complete() {
    if (selected?.task.task.status !== "open" || busy || stale || uncertain)
      return;
    const tracking = selected.task.task.tracking;
    if (tracking && tracking.type !== "boolean") prompt = true;
    else void write("tasks_complete", {});
  }

  async function checkCompletion() {
    const request = uncertain;
    if (busy || !request || !listArgs || !canUse("tasks_inspect")) return;
    const vault = listArgs.vault_id;
    busy = true;
    error = "";
    try {
      const result = await bridge.call("tasks_inspect", {
        vault_id: vault,
        request_id: request.id,
      });
      const receipt = toolValue(result, completionResult).operation;
      if (
        receipt?.request_id === request.id &&
        receipt.path === request.path &&
        receipt.disposition === "completed"
      ) {
        await afterWrite(vault, request.path, "Task completed.");
        uncertain = undefined;
      } else {
        error = `Completion status: ${receipt?.disposition ?? "no receipt found"}. Use Tasks recovery in Lapis to review this request.`;
      }
    } catch {
      error =
        "Completion status is unavailable. Use Tasks recovery in Lapis to review this request.";
    } finally {
      busy = false;
    }
  }
</script>

<main>
  <header class="toolbar">
    <h1>Lapis Tasks</h1>
    {#if canCall && listArgs}
      <Button size="sm" variant="outline" disabled={busy} onclick={refresh}
        >Refresh tasks</Button
      >
    {/if}
  </header>
  {#if notice}<p class="message" role="status">{notice}</p>{/if}
  {#if error}<p class="message" role="alert">{error}</p>{/if}
  {#if uncertain}
    <p>Request ID: <code>{uncertain.id}</code></p>
    {#if canUse("tasks_inspect")}
      <Button
        size="sm"
        variant="outline"
        disabled={busy}
        onclick={checkCompletion}>Check completion status</Button
      >
    {/if}
  {/if}
  {#if list}
    {@const rows = list.items.filter((item) => item.kind === "task")}
    {#if rows.length}
      <ul aria-label="Tasks" inert={busy || Boolean(uncertain)}>
        {#each rows as row (row.documentPath)}
          <li>
            <ApplicationTaskLine
              task={taskLine(row)}
              titleMode={canCall && listArgs ? "button" : "text"}
              onTitleClick={() => void select(row.documentPath)}
            />
          </li>
        {/each}
      </ul>
    {:else}
      <p>No tasks in this view.</p>
    {/if}
    {#if list.truncated}<p>Showing a limited set of tasks.</p>{/if}
  {:else if busy}
    <p role="status">Reading tasks…</p>
  {/if}

  {#if selected}
    <TaskFields
      {selected}
      disabled={busy || stale || Boolean(uncertain)}
      {canUse}
      {write}
      {complete}
    />
  {/if}
  {#if selectedPath && !uncertain && (stale || !selected) && canCall}
    <Button
      size="sm"
      variant="outline"
      disabled={busy}
      onclick={() => select(selectedPath)}>Reload saved task</Button
    >
  {/if}
  {#if prompt && selected}
    <TaskTrackingCompletionPrompt
      task={promptTask(selected)}
      onCancel={() => (prompt = false)}
      onConfirm={(value) => write("tasks_complete", { trackedValue: value })}
    />
  {/if}
</main>

<style>
  ul {
    padding: 0;
    margin: 0;
    list-style: none;
  }

  li + li {
    border-block-start: 1px solid var(--ui-border, #8885);
  }
</style>
