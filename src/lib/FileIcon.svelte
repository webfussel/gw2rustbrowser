<script lang="ts">
  import { iconFor } from "./icons";

  let { type, size = 15 }: { type: string | null | undefined; size?: number } = $props();

  const spec = $derived(iconFor(type));
</script>

<span class="icon tone-{spec.tone}" title={spec.label} aria-label={spec.label} role="img">
  <spec.icon {size} strokeWidth={1.75} />
</span>

<style>
  .icon {
    display: inline-grid;
    place-items: center;
    flex: none;
    color: var(--tone);
  }

  .tone-image { --tone: var(--tone-image); }
  .tone-sound { --tone: var(--tone-sound); }
  .tone-model { --tone: var(--tone-model); }
  .tone-animation { --tone: var(--tone-animation); }
  .tone-text { --tone: var(--tone-text); }
  .tone-map { --tone: var(--tone-map); }
  .tone-font { --tone: var(--tone-font); }
  .tone-video { --tone: var(--tone-video); }
  .tone-code { --tone: var(--tone-code); }
  .tone-data { --tone: var(--tone-data); }
  .tone-unknown { --tone: var(--muted); }
</style>
