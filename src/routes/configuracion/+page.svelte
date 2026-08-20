<script lang="ts">
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
</script>

<svelte:head>
  <title>configuración — idioteque</title>
</svelte:head>

<main class="settings">
  <header>
    <a href="/" class="back" aria-label="Volver">
      <ArrowLeft size={18} strokeWidth={1.75} aria-hidden="true" />
    </a>
    <h1>Configuración</h1>
  </header>
</main>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: auto;
    padding: 2rem 2rem 3rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  h1 {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 600;
  }

  .back {
    display: inline-flex;
    box-sizing: border-box;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: 2.35rem;
    height: 2.35rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-hover);
    color: var(--text);
    text-decoration: none;
  }

  .back:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
