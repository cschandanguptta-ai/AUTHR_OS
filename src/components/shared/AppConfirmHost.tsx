// AppConfirmHost — single mount point for the app-level ConfirmDialog
// (confirmStore requests from kebab menu / command palette) and the global
// UndoToast (R-06).

import { ConfirmDialog } from './ConfirmDialog';
import { UndoToast } from './UndoToast';
import { useConfirmStore } from '../../stores/confirmStore';

export function AppConfirmHost() {
  const req = useConfirmStore((s) => s.req);
  const close = useConfirmStore((s) => s.close);

  return (
    <>
      <ConfirmDialog
        open={req !== null}
        title={req?.title ?? ''}
        body={req?.body ?? ''}
        confirmLabel={req?.confirmLabel}
        typeToConfirm={req?.typeToConfirm}
        onConfirm={() => {
          req?.onConfirm();
          close();
        }}
        onClose={close}
      />
      <UndoToast />
    </>
  );
}
