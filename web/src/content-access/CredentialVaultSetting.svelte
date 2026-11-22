<script lang="ts" module>
  import type { WorkspaceCustomSetting } from "@lapis-notes/api";
  import type { ContentAccessModel } from "./model";

  export type CredentialVaultField = WorkspaceCustomSetting & {
    contentAccess: ContentAccessModel;
  };
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { WorkspaceCustomSettingProps } from "@lapis-notes/api";
  import ScopePicker, {
    maxCredentialVaults,
    type ScopeChoice,
  } from "./ScopePicker.svelte";

  let {
    id,
    field,
    value,
    setValue,
    disabled,
    busy,
  }: WorkspaceCustomSettingProps = $props();
  let choices = $state<ScopeChoice[]>([]);
  const settings = $derived(field as CredentialVaultField);
  const selected = $derived(Array.isArray(value) ? value : []);

  onMount(() => {
    const model = settings.contentAccess;
    const update = () => {
      choices = model.options.map(({ value: id, label }) => ({ id, label }));
    };
    update();
    return model.subscribe(update);
  });
</script>

<ScopePicker
  label={field.title}
  searchLabel="Search vaults"
  inputId={`setting-${id}`}
  {choices}
  {selected}
  limit={maxCredentialVaults}
  disabled={disabled || busy}
  onChange={async (ids) => {
    await setValue(ids);
  }}
/>
