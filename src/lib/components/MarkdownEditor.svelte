<script lang="ts">
  import { onMount } from "svelte";
  import { EditorView, basicSetup } from "codemirror";
  import { markdown } from "@codemirror/lang-markdown";
  import { languages } from "@codemirror/language-data";

  let {
    content,
    onChange,
  }: {
    content: string;
    onChange: (contents: string) => void;
  } = $props();

  let host: HTMLDivElement;
  let view = $state<EditorView>();
  // Set while we push disk content into the editor, so the round trip back to
  // the store is not mistaken for a user edit.
  let applying = false;

  onMount(() => {
    const editor = new EditorView({
      doc: content,
      parent: host,
      extensions: [
        basicSetup,
        markdown({ codeLanguages: languages }),
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
          if (!update.docChanged || applying) return;
          onChange(update.state.doc.toString());
        }),
      ],
    });

    view = editor;

    return () => {
      view = undefined;
      editor.destroy();
    };
  });

  $effect(() => {
    const next = content;
    if (!view || view.state.doc.toString() === next) return;

    applying = true;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: next },
      selection: { anchor: 0 },
    });
    applying = false;
  });
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    height: 100%;
    overflow: hidden;
  }

  .editor :global(.cm-editor) {
    height: 100%;
  }

  .editor :global(.cm-editor.cm-focused) {
    outline: none;
  }

  .editor :global(.cm-scroller) {
    font-family: var(--font-mono);
    font-size: 0.9rem;
    line-height: 1.65;
  }

  .editor :global(.cm-content) {
    padding: 1.25rem 0;
  }
</style>
