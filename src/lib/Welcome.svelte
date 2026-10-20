<script lang="ts">
  import { openArchive } from "./state.svelte";
</script>

<div class="welcome">
  <div class="card">
    <svg viewBox="0 0 24 24" width="56" height="56" aria-hidden="true">
      <path d="M12 2 21 7v10l-9 5-9-5V7z" fill="var(--accent)" />
      <path d="M12 2 21 7l-9 5-9-5z" fill="#fff" fill-opacity=".25" />
      <path d="M12 12v10" stroke="#000" stroke-opacity=".2" />
    </svg>
    <h2>Open a Guild Wars 2 archive</h2>
    <p class="muted">Choose your <span class="mono">Gw2.dat</span> or drop it onto this window. The first scan takes a moment; after that the index is cached and opening is instant.</p>
    <button class="btn primary" onclick={() => openArchive()}>Open Gw2.dat…</button>
  </div>
</div>

<style>
  .welcome {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 24px;
  }

  .card {
    max-width: 440px;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  h2 {
    margin: 12px 0 0;
    font-size: 20px;
    font-weight: 600;
  }

  p {
    margin: 4px 0 16px;
  }
</style>
