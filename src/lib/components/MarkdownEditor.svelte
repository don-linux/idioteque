<script lang="ts">
  import { onMount } from "svelte";
  import { markdown } from "@codemirror/lang-markdown";
  import { languages } from "@codemirror/language-data";
  import { EditorState } from "@codemirror/state";
  import { EditorView, basicSetup } from "codemirror";
  import { editorSession } from "$lib/editor-session.svelte";
  import { externalDocumentSpec } from "$lib/editor-sync";

  let {
    path,
    content,
    contentReady,
    onChange,
  }: {
    path: string;
    content: string;
    contentReady: boolean;
    onChange: (contents: string) => void;
  } = $props();

  let host: HTMLDivElement | undefined;
  let view: EditorView | undefined;
  let ready = $state(false);
  let applying = false;
  let activePath = "";

  function extensions() {
    return [
      basicSetup,
      markdown({ codeLanguages: languages }),
      EditorView.lineWrapping,
      EditorView.updateListener.of((update) => {
        editorSession.sync(update.state);
        if (!update.docChanged || applying) return;
        onChange(update.state.doc.toString());
      }),
    ];
  }

  function createState(doc: string): EditorState {
    return EditorState.create({
      doc,
      extensions: extensions(),
    });
  }

  onMount(() => {
    if (!host) return;

    const editor = new EditorView({
      state: createState(contentReady ? content : ""),
      parent: host,
    });

    view = editor;
    activePath = path;
    editorSession.attach(editor);
    ready = true;

    return () => {
      if (activePath) editorSession.saveState(activePath, editor.state);
      editorSession.detach();
      ready = false;
      view = undefined;
      editor.destroy();
    };
  });

  $effect(() => {
    const nextPath = path;
    const nextContent = content;
    const readyContent = contentReady;
    if (!ready || !view) return;

    if (nextPath !== activePath) {
      if (activePath) editorSession.saveState(activePath, view.state);

      const saved = editorSession.takeState(nextPath);
      applying = true;
      view.setState(saved ?? createState(readyContent ? nextContent : ""));
      applying = false;
      activePath = nextPath;
      editorSession.sync(view.state);
    }

    if (!readyContent || view.state.doc.toString() === nextContent) return;

    const scroll = view.scrollDOM.scrollTop;

    applying = true;
    view.dispatch(externalDocumentSpec(view.state, nextContent));
    view.scrollDOM.scrollTop = scroll;
    applying = false;
    editorSession.sync(view.state);
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
