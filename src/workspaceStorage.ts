import type { Draft } from './components/Composer';
import type { AgentMode, DebateAgent } from './ipc';

let database: Promise<IDBDatabase> | undefined;
function draftsDb() {
  return (database ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('quill-workspace', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('drafts');
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => {
      database = undefined;
      reject(request.error);
    };
  }));
}
export async function loadDrafts(
  ids: number[],
): Promise<Record<number, Draft>> {
  const db = await draftsDb();
  const allowed = new Set([0, ...ids]);
  return new Promise((resolve, reject) => {
    const tx = db.transaction('drafts', 'readwrite');
    const request = tx.objectStore('drafts').openCursor();
    const result: Record<number, Draft> = {};
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor) return;
      if (allowed.has(Number(cursor.key)))
        result[Number(cursor.key)] = cursor.value;
      else cursor.delete();
      cursor.continue();
    };
    tx.oncomplete = () => resolve(result);
    tx.onabort = () => reject(tx.error);
  });
}
export async function storeDrafts(
  before: Record<number, Draft>,
  after: Record<number, Draft>,
  keep: boolean,
) {
  const db = await draftsDb();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction('drafts', 'readwrite'),
      store = tx.objectStore('drafts');
    if (!keep) store.clear();
    else
      for (const id of new Set([
        ...Object.keys(before),
        ...Object.keys(after),
      ])) {
        const draft = after[+id];
        if (draft === before[+id]) continue;
        if (draft && (draft.text || draft.files.length)) store.put(draft, +id);
        else store.delete(+id);
      }
    tx.oncomplete = () => resolve();
    tx.onabort = () => reject(tx.error);
  });
}
export interface Team {
  name: string;
  mode: AgentMode;
  agents: DebateAgent[];
}
function key(account: string | null) {
  return 'quill.teams.v1.' + JSON.stringify(account);
}
export function readTeams(account: string | null): Team[] {
  try {
    const entries: unknown = JSON.parse(
      localStorage.getItem(key(account)) ?? '[]',
    );
    return Array.isArray(entries)
      ? entries.filter(
          (t) =>
            t &&
            typeof t.name === 'string' &&
            ['roundtable', 'parallel'].includes(t.mode) &&
            Array.isArray(t.agents) &&
            t.agents.length >= 2 &&
            t.agents.length <= 8 &&
            t.agents.every(
              (a: DebateAgent) =>
                a &&
                typeof a.model === 'string' &&
                typeof a.name === 'string' &&
                typeof a.personality === 'string',
            ),
        )
      : [];
  } catch {
    return [];
  }
}
export function writeTeams(account: string | null, teams: Team[]) {
  localStorage.setItem(key(account), JSON.stringify(teams));
}
