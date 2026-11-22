<script lang="ts">
  let {
    alt,
    ref = $bindable(null),
  }: {
    alt?: string;
    ref?: HTMLElement | null;
  } = $props();
</script>

<span bind:this={ref}>{alt?.trim() || "Embedded media"}</span>
