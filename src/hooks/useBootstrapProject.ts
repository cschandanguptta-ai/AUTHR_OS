// AUTHR_OS — bootstrap: once a bundle is open, hydrate every store via IPC.

import { useEffect } from 'react';
import { useProjectStore } from '../stores/projectStore';
import { useManuscriptStore } from '../stores/manuscriptStore';
import { useVoiceCastStore } from '../stores/voiceCastStore';
import { useLoreStore } from '../stores/loreStore';
import { useThreadStore } from '../stores/threadStore';
import { toastError } from '../stores/uiStore';
import * as api from '../ipc/api';

export function useBootstrapProject() {
  const status = useProjectStore((s) => s.status);
  const sessionNonce = useProjectStore((s) => s.sessionNonce);

  useEffect(() => {
    if (status !== 'open') return;
    let alive = true;

    (async () => {
      const manuscript = useManuscriptStore.getState();
      try {
        await manuscript.loadBooks();
        if (!alive) return;
        await manuscript.loadChapters();
        if (!alive) return;

        // Fresh bundles come with a scaffold book from project_init; if the
        // bundle is empty, create the working title book so the binder is usable.
        let books = useManuscriptStore.getState().books;
        const { manifest, defaultBookId } = useProjectStore.getState();
        if (!books.length) {
          try {
            const created = await api.bookCreate({
              title: manifest?.title ?? 'Untitled Novel',
              author: manifest?.author,
            });
            if (alive) {
              await manuscript.loadBooks();
              books = useManuscriptStore.getState().books;
              void created;
            }
          } catch (err) {
            toastError('Could not seed the first book', err);
          }
        }

        // Select the first chapter of the scaffold book if present.
        const chapters = useManuscriptStore.getState().chapters;
        const firstBook =
          (defaultBookId && books.find((b) => b.id === defaultBookId)?.id) ?? books[0]?.id ?? null;
        const firstChapter =
          chapters.find((c) => (firstBook != null ? c.book_id === firstBook : true)) ?? null;
        manuscript.selectChapter(firstChapter ? firstChapter.id : null);
        if (firstBook != null) {
          useManuscriptStore.setState({ activeBookId: firstBook });
        }

        if (!alive) return;
        void useVoiceCastStore.getState().loadCharacters();
        void useLoreStore.getState().loadLore();
        void useThreadStore.getState().loadThreads();
      } catch (err) {
        toastError('Project bootstrap failed', err);
      }
    })();

    return () => {
      alive = false;
    };
  }, [status, sessionNonce]);
}
