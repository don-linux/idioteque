import {
  Transaction,
  type EditorState,
  type TransactionSpec,
} from "@codemirror/state";

export function externalDocumentSpec(
  state: EditorState,
  next: string,
): TransactionSpec {
  return {
    changes: { from: 0, to: state.doc.length, insert: next },
    selection: { anchor: Math.min(state.selection.main.head, next.length) },
    annotations: Transaction.addToHistory.of(false),
  };
}
