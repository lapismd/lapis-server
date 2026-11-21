<script lang="ts" module>
  export type ScopeChoice = { id: string; label: string; description?: string };
  export const maxCredentialVaults = 20;
</script>

<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import X from "@lucide/svelte/icons/x";
  import * as Button from "@lapismd/design-core/shadcn/button";
  import * as CommandView from "@lapismd/design-core/shadcn/command-view";

  let {
    label,
    searchLabel,
    inputId,
    choices,
    selected,
    onChange,
    disabled = false,
    limit,
  }: {
    label: string;
    searchLabel: string;
    inputId?: string;
    choices: ScopeChoice[];
    selected: string[];
    onChange: (ids: string[]) => void | Promise<void>;
    disabled?: boolean;
    limit?: number;
  } = $props();
  let query = $state("");
  let changing = $state(false);
  const atLimit = $derived(limit !== undefined && selected.length >= limit);

  async function toggle(id: string) {
    if (disabled || changing) return;
    const remove = selected.includes(id);
    if (!remove && atLimit) return;
    changing = true;
    try {
      await onChange(
        remove ? selected.filter((entry) => entry !== id) : [...selected, id],
      );
    } finally {
      changing = false;
    }
  }
</script>

<div class="scope-picker">
  {#if selected.length}
    <ul class="selection-summary" aria-label={`${label} selected`}>
      {#each choices.filter( (choice) => selected.includes(choice.id), ) as choice (choice.id)}
        <li>
          <Button.Root
            type="button"
            size="sm"
            variant="secondary"
            {disabled}
            aria-label={`Remove ${choice.label}`}
            onclick={() => toggle(choice.id)}
          >
            {choice.label}
            <X aria-hidden="true" data-icon="inline-end" />
          </Button.Root>
        </li>
      {/each}
    </ul>
  {/if}
  <CommandView.Root {label} shouldFilter={false}>
    <CommandView.Input
      bind:value={query}
      id={inputId}
      aria-label={searchLabel}
      placeholder={`${searchLabel}…`}
      {disabled}
      autocomplete="off"
    />
    <CommandView.List aria-label={label} aria-multiselectable="true">
      <CommandView.Empty>No matching options.</CommandView.Empty>
      <CommandView.Group>
        {#each choices.filter( (choice) => `${choice.label} ${choice.description ?? ""}`
              .toLowerCase()
              .includes(query.trim().toLowerCase()), ) as choice (choice.id)}
          {@const checked = selected.includes(choice.id)}
          <CommandView.Item
            value={choice.id}
            disabled={disabled || changing || (!checked && atLimit)}
            onSelect={() => toggle(choice.id)}
          >
            {#snippet child({ props })}
              <div {...props} aria-selected={checked} class="scope-option">
                <CommandView.ItemIcon>
                  <span class="selection-mark" class:checked aria-hidden="true">
                    {#if checked}<Check />{/if}
                  </span>
                </CommandView.ItemIcon>
                <CommandView.ItemLabel>{choice.label}</CommandView.ItemLabel>
                {#if choice.description}
                  <CommandView.ItemDescription
                    >{choice.description}</CommandView.ItemDescription
                  >
                {/if}
              </div>
            {/snippet}
          </CommandView.Item>
        {/each}
      </CommandView.Group>
    </CommandView.List>
    {#if limit !== undefined && selected.length >= limit}
      <CommandView.Footer
        >Up to {limit} vaults can be shared.</CommandView.Footer
      >
    {/if}
  </CommandView.Root>
</div>

<style>
  .scope-picker {
    width: 100%;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--background);
    --ui-command-view-list-max-height: 13rem;
  }

  .scope-picker
    :global(
      [data-ui-part="input-wrapper"]
        [data-ui-component="input-group"]
        [data-ui-component="input"]
    ) {
    width: 100%;
    min-width: 0;
    border: 0;
    background: transparent;
  }

  .selection-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    list-style: none;
    padding: 0.375rem;
    margin: 0;
    border-block-end: 1px solid var(--border);
  }

  .scope-option {
    align-items: center;
  }

  .selection-mark {
    display: grid;
    place-items: center;
    width: 1rem;
    height: 1rem;
    border: 1px solid var(--input);
    border-radius: 0.25rem;
  }

  .selection-mark.checked {
    border-color: var(--primary);
    background: var(--primary);
    color: var(--primary-foreground);
  }

  .selection-mark :global(svg) {
    width: 0.75rem;
    height: 0.75rem;
  }
</style>
