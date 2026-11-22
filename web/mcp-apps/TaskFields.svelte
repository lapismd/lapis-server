<script lang="ts">
  import { ApplicationTaskWhenPicker } from "@lapis-notes/lapis-plugin-tasks";
  import { Button } from "@lapismd/design-core/shadcn/button";
  import type { SavedTask } from "./task-data";

  let {
    selected,
    disabled,
    canUse,
    write,
    complete,
  }: {
    selected: SavedTask;
    disabled: boolean;
    canUse: (name: string) => boolean;
    write: (name: string, fields: Record<string, unknown>) => Promise<void>;
    complete: () => void;
  } = $props();

  const task = $derived(selected.task.task);
  async function changePlan(date: string | null) {
    await write("tasks_plan", {
      plan: date
        ? { ...task.plan, date, at: task.plan?.at ?? "anytime" }
        : null,
    });
  }
</script>

<section aria-label="Selected task" aria-busy={disabled}>
  <h2>{selected.task.title}</h2>
  <p class="status">{task.status}</p>
  <dl>
    <dt>Planned date</dt>
    <dd>
      {#if canUse("tasks_plan") && !disabled}
        <div class="plan-picker">
          <input
            type="date"
            aria-label="Planned date"
            value={task.plan?.date ?? ""}
            onchange={(event) => {
              const date = event.currentTarget.value;
              event.currentTarget.value = task.plan?.date ?? "";
              void changePlan(date || null);
            }}
          />
          {#if task.plan}
            <Button size="sm" variant="ghost" onclick={() => changePlan(null)}
              >Clear planned date</Button
            >
          {/if}
        </div>
      {:else}
        {task.plan?.date ?? "None"}
      {/if}
    </dd>
    <dt>Deadline</dt>
    <dd>
      {#if canUse("tasks_deadline") && !disabled}
        <ApplicationTaskWhenPicker
          mode="due"
          scheduledAt={task.deadline}
          onSelectDate={(deadline) => write("tasks_deadline", { deadline })}
          onClear={() => write("tasks_deadline", { deadline: null })}
        />
      {:else}
        {task.deadline ?? "None"}
      {/if}
    </dd>
    <dt>Inbox</dt>
    <dd>
      {#if canUse("tasks_inbox")}
        <label>
          <input
            type="checkbox"
            checked={task.inbox ?? false}
            {disabled}
            onchange={(event) => {
              const inbox = event.currentTarget.checked;
              event.currentTarget.checked = task.inbox ?? false;
              void write("tasks_inbox", { inbox });
            }}
          />
          In Inbox
        </label>
      {:else}
        {task.inbox ? "In Inbox" : "Outside Inbox"}
      {/if}
    </dd>
  </dl>
  {#if task.status === "open" && canUse("tasks_complete")}
    <Button size="sm" {disabled} onclick={complete}>Complete</Button>
  {/if}
</section>

<style>
  section {
    border-block-start: 1px solid var(--ui-border, #8885);
    padding-block-start: 0.75rem;
    margin-block-start: 0.75rem;
  }

  h2 {
    font-size: 1rem;
  }
  .status {
    text-transform: capitalize;
  }

  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    align-items: center;
    gap: 0.5rem 0.75rem;
  }

  dd {
    margin: 0;
  }

  .plan-picker {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.25rem;
  }

  input[type="date"] {
    max-inline-size: 100%;
    padding: 0.35rem;
    border: 1px solid var(--ui-border, #8885);
    border-radius: var(--radius, 0.5rem);
  }
</style>
