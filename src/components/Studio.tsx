import { useEffect, useRef, useState } from 'react';
import { call, type Chat, type Turn, type Workspace } from '../ipc';

export type StudioTab = 'Resume' | 'Memory' | 'Branches' | 'Draft';
interface Suggestion {
  before: string;
  after: string;
}
function edits(text: string): Suggestion[] {
  try {
    const value: unknown = JSON.parse(
      text.trim().replace(/^```(?:json)?\s*|\s*```$/g, ''),
    );
    if (!Array.isArray(value)) return [];
    return value.filter(
      (v): v is Suggestion =>
        v &&
        typeof v.before === 'string' &&
        v.before.length > 0 &&
        typeof v.after === 'string',
    );
  } catch {
    return [];
  }
}
type Props = {
  chat: number;
  chats: Chat[];
  turns: Turn[];
  tab: StudioTab;
  tabChanged: (t: StudioTab) => void;
  source: Turn | null;
  clearSource: () => void;
  close: () => void;
  navigate: (id: number, turn: number | null) => void;
  compose: (id: number, text: string, agents?: boolean) => void;
  busy: boolean;
};

function passages(t: Turn): { name: string; text: string }[] {
  if (t.debate?.ballot && t.debate.ballot.winner === null) return [];
  if (t.debate)
    return t.debate.entries
      .filter((e) => e.status === 'complete')
      .map((e) => ({ name: t.debate!.agents[e.seat].name, text: e.text }));
  return t.reply ? [{ name: 'Quill', text: t.reply }] : [];
}
// Keep unsaved project edits in memory when switching chats in the same window.
const unsaved = new Map<number, { text: string; revision: number }>();
export function Studio({
  chat,
  chats,
  turns,
  tab,
  tabChanged,
  source,
  clearSource,
  close,
  navigate,
  compose,
  busy,
}: Props) {
  const [data, setData] = useState<Workspace | null>(null);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const [kind, setKind] = useState('Decision');
  const [note, setNote] = useState('');
  const [edit, setEdit] = useState<number | null>(null);
  const [origin, setOrigin] = useState<number | null>(null);
  const [other, setOther] = useState('');
  const [compare, setCompare] = useState<number | null>(null);
  const [counterpart, setCounterpart] = useState<Turn[]>([]);
  const [local, setLocal] = useState('');
  const [revision, setRevision] = useState(0);
  const [suggestionKey, setSuggestionKey] = useState('');
  const [proposal, setProposal] = useState('');
  const [range, setRange] = useState<[number, number]>([0, 0]);
  const [picked, setPicked] = useState('');
  const [ready, setReady] = useState(false);
  const lock = useRef(false);
  const editor = useRef<HTMLTextAreaElement>(null);
  const related = [
    ...(data?.parent ? [data.parent] : []),
    ...(data?.children ?? []),
  ];
  const proposals = turns
    .flatMap((t) =>
      passages(t).map((p, i) => ({
        ...p,
        key: t.id + '-' + i,
        prompt: t.prompt,
      })),
    )
    .reverse();
  const title = (id: number) => chats.find((c) => c.id === id)?.title ?? 'Chat';
  useEffect(() => {
    let live = true;
    call('workspace', { chat })
      .then((d) => {
        if (live) {
          setData(d);
          const pending = unsaved.get(chat);
          setLocal(pending?.text ?? d.draft);
          setRevision(pending?.revision ?? d.revision);
          setReady(true);
        }
      })
      .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [chat]);
  useEffect(() => {
    if (source) {
      setEdit(null);
      setOrigin(source.id);
      setKind('Decision');
      setNote(
        (source.debate?.ballot?.winner === null
          ? source.prompt
          : source.reply || source.prompt
        ).slice(0, 2000),
      );
    }
  }, [source]);
  useEffect(() => {
    let live = true;
    setCounterpart([]);
    setPicked('');
    if (compare !== null)
      call('list_turns', { chat: compare })
        .then((t) => live && setCounterpart(t))
        .catch((e) => live && setError(String(e)));
    return () => {
      live = false;
    };
  }, [compare]);
  async function write(action: () => Promise<void>) {
    if (lock.current) return;
    lock.current = true;
    setSaving(true);
    setError('');
    try {
      await action();
      setData(await call('workspace', { chat }));
    } catch (e) {
      setError(String(e));
    } finally {
      lock.current = false;
      setSaving(false);
    }
  }
  function reset() {
    setEdit(null);
    setOrigin(null);
    setNote('');
    clearSource();
  }
  const dirty = ready && local !== data?.draft;
  useEffect(() => {
    if (!ready) return;
    if (dirty) unsaved.set(chat, { text: local, revision });
    else unsaved.delete(chat);
  }, [ready, dirty, chat, local, revision]);
  const blocked = busy || saving;
  const [discard, setDiscard] = useState(false);
  function leave() {
    if (dirty) {
      setDiscard(true);
    } else close();
  }
  function save() {
    return write(async () => {
      await call('save_draft', { chat, text: local, revision });
      setRevision(revision + 1);
      setDiscard(false);
    });
  }
  const projectName = data?.projects.find((p) => p.id === data.project)?.name;
  return (
    <aside className="studio-panel" aria-label="Project workspace">
      <header>
        <div>
          <small>Workspace</small>
          <h2>{projectName ?? title(chat)}</h2>
        </div>
        <button aria-label="Close workspace" onClick={leave}>
          ×
        </button>
      </header>
      <nav aria-label="Workspace sections">
        {(['Resume', 'Memory', 'Branches', 'Draft'] as const).map((t) => (
          <button
            key={t}
            aria-pressed={t === tab}
            onClick={() => tabChanged(t)}
          >
            {t}
            {t === 'Draft' && dirty ? ' •' : ''}
          </button>
        ))}
      </nav>
      {error && <p role="alert">{error}</p>}
      {discard && (
        <section>
          <p>Save your working draft before closing?</p>
          <button disabled={blocked} onClick={save}>
            Save draft
          </button>{' '}
          <button
            onClick={() => {
              unsaved.delete(chat);
              close();
            }}
          >
            Discard edits
          </button>{' '}
          <button onClick={() => setDiscard(false)}>Keep editing</button>
        </section>
      )}
      {!data ? (
        <p>Loading project…</p>
      ) : (
        <div className="studio-scroll">
          {tab === 'Resume' && (
            <>
              {(['Decision', 'Tried', 'Open question'] as const).map((k) => (
                <section key={k}>
                  <h3>
                    {k === 'Decision'
                      ? 'Decisions'
                      : k === 'Tried'
                        ? 'Already tried'
                        : 'Open questions'}
                  </h3>
                  {data.pins.filter((p) => p.kind === k).length === 0 ? (
                    <small>No saved {k.toLowerCase()} yet.</small>
                  ) : (
                    data.pins
                      .filter((p) => p.kind === k)
                      .map((p) => (
                        <div key={p.id}>
                          <p>{p.text}</p>
                          {p.source && p.chat && (
                            <button
                              disabled={blocked || dirty}
                              onClick={() => navigate(p.chat!, p.source)}
                            >
                              Open source message
                            </button>
                          )}
                        </div>
                      ))
                  )}
                </section>
              ))}
              <section>
                <h3>Project recap</h3>
                <button
                  disabled={blocked || dirty || !data.recent.length}
                  onClick={() =>
                    compose(
                      chat,
                      'Summarize these project exchanges under Decisions, Already tried, and Open questions. Cite each item using the exact Markdown source link supplied for that message. Flag unknowns.\n\n' +
                        data.recent
                          .map(
                            (t) =>
                              '[Message ' +
                              t.turn +
                              '](#quill-message-' +
                              t.chat +
                              '-' +
                              t.turn +
                              ') ' +
                              t.title +
                              '\nYou: ' +
                              t.prompt +
                              '\nReply: ' +
                              t.reply,
                          )
                          .join('\n\n'),
                    )
                  }
                >
                  Prepare recap in chat
                </button>
              </section>
              <section>
                <h3>Recent exchanges</h3>
                {data.recent.map((t) => (
                  <button
                    disabled={blocked || dirty}
                    className="recent-turn"
                    key={t.turn}
                    onClick={() => navigate(t.chat, t.turn)}
                  >
                    <small>
                      {t.title} · {t.status}
                    </small>
                    <b>{t.prompt || 'Attachment'}</b>
                  </button>
                ))}
              </section>
              <section>
                <h3>Blind comparison record</h3>
                {data.scores.length ? (
                  data.scores.map((s) => (
                    <p key={s.category + s.model}>
                      <small>{s.category}</small>
                      {s.model} · {s.wins} picks across {s.ballots} answers
                    </p>
                  ))
                ) : (
                  <small>
                    Vote in a blind Parallel run to start your record.
                  </small>
                )}
              </section>
            </>
          )}
          {tab === 'Memory' && (
            <>
              <section>
                <h3>Project chats</h3>
                <p>
                  {(data.members.length ? data.members : [chat])
                    .map(title)
                    .join(' · ')}
                </p>
                <label>
                  Add a chat
                  <select
                    value={other}
                    onChange={(e) => setOther(e.target.value)}
                  >
                    <option value="">Choose a chat</option>
                    {chats
                      .filter(
                        (c) => c.id !== chat && !data.members.includes(c.id),
                      )
                      .map((c) => (
                        <option key={c.id} value={c.id}>
                          {c.title}
                        </option>
                      ))}
                  </select>
                </label>
                <button
                  disabled={blocked || !other}
                  onClick={() =>
                    write(async () => {
                      await call('join_project', {
                        chat: Number(other),
                        other: chat,
                      });
                      setOther('');
                    })
                  }
                >
                  Include in project
                </button>
              </section>
              <section>
                <h3>Pinned context</h3>
                <small>
                  These notes accompany new requests in every chat in this
                  project.
                </small>
                {data.pins.map((p) => (
                  <div className="pin-card" key={p.id}>
                    <small>{p.kind}</small>
                    <p>{p.text}</p>
                    <div className="studio-actions">
                      {p.source && p.chat && (
                        <button
                          disabled={blocked || dirty}
                          onClick={() => navigate(p.chat!, p.source)}
                        >
                          Source
                        </button>
                      )}
                      <button
                        disabled={blocked}
                        onClick={() => {
                          clearSource();
                          setEdit(p.id);
                          setOrigin(p.source);
                          setKind(p.kind);
                          setNote(p.text);
                        }}
                      >
                        Edit
                      </button>
                      <button
                        disabled={blocked}
                        onClick={() =>
                          write(async () => {
                            await call('remove_pin', { chat, id: p.id });
                            if (edit === p.id) reset();
                          })
                        }
                      >
                        Remove
                      </button>
                    </div>
                  </div>
                ))}
              </section>
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  void write(async () => {
                    await call('save_pin', {
                      chat,
                      id: edit,
                      kind,
                      text: note,
                      source: origin,
                    });
                    reset();
                  });
                }}
              >
                <label>
                  Note type
                  <select
                    value={kind}
                    onChange={(e) => setKind(e.target.value)}
                  >
                    {['Decision', 'Tried', 'Open question', 'Memory'].map(
                      (k) => (
                        <option key={k}>{k}</option>
                      ),
                    )}
                  </select>
                </label>
                <label>
                  {origin ? 'Note linked to message ' + origin : 'Project note'}
                  <textarea
                    rows={4}
                    maxLength={2000}
                    value={note}
                    onChange={(e) => setNote(e.target.value)}
                  />
                </label>
                <button disabled={blocked || !note.trim()}>
                  {edit ? 'Update note' : 'Pin note'}
                </button>{' '}
                {note && (
                  <button type="button" onClick={reset}>
                    Clear
                  </button>
                )}
              </form>
            </>
          )}
          {tab === 'Branches' && (
            <>
              <p>Use “Try both” under a message to branch from that point.</p>
              <label>
                Compare this chat with
                <select
                  value={compare ?? ''}
                  onChange={(e) => setCompare(Number(e.target.value) || null)}
                >
                  <option value="">Choose a branch</option>
                  {related.map((id) => (
                    <option key={id} value={id}>
                      {title(id)}
                    </option>
                  ))}
                </select>
              </label>
              {compare !== null && (
                <>
                  <div className="branch-grid">
                    {[
                      [chat, turns],
                      [compare, counterpart],
                    ].map((pair) => {
                      const id = pair[0] as number,
                        items = pair[1] as Turn[];
                      return (
                        <section key={id}>
                          <h3>{title(id)}</h3>
                          {items.map((t) => (
                            <details
                              key={t.id}
                              open={t.id === items.at(-1)?.id}
                            >
                              <summary>{t.prompt || 'Attachment'}</summary>
                              {passages(t).map((p, i) => (
                                <div key={i}>
                                  <p>{p.text}</p>
                                  <button
                                    onClick={() =>
                                      setPicked((prev) =>
                                        [prev, p.text]
                                          .filter(Boolean)
                                          .join('\n\n'),
                                      )
                                    }
                                  >
                                    Keep passage
                                  </button>
                                </div>
                              ))}
                            </details>
                          ))}
                        </section>
                      );
                    })}
                  </div>
                  <label>
                    Bring back to original
                    <textarea
                      rows={4}
                      value={picked}
                      onChange={(e) => setPicked(e.target.value)}
                    />
                  </label>
                  <button
                    disabled={blocked || dirty || !picked.trim()}
                    onClick={() =>
                      compose(data.parent === compare ? compare : chat, picked)
                    }
                  >
                    Use in original composer
                  </button>
                </>
              )}
            </>
          )}
          {tab === 'Draft' && (
            <>
              <label>
                Working draft
                <textarea
                  ref={editor}
                  rows={10}
                  value={local}
                  onChange={(e) => {
                    setLocal(e.target.value);
                    setRange([e.target.selectionStart, e.target.selectionEnd]);
                  }}
                  onSelect={(e) =>
                    setRange([
                      e.currentTarget.selectionStart,
                      e.currentTarget.selectionEnd,
                    ])
                  }
                />
              </label>
              <div className="studio-actions">
                <button disabled={blocked || !dirty} onClick={save}>
                  Save draft
                </button>
                <button
                  disabled={blocked || dirty || !local.trim()}
                  onClick={() =>
                    compose(
                      chat,
                      'Review this working draft. Return only a JSON array of individual edits. Each edit has "before" (an exact unique passage from the draft) and "after" (replacement text). Preserve everything else.\n\n' +
                        local,
                      true,
                    )
                  }
                >
                  Ask agents for edits
                </button>
              </div>
              <section>
                <h3>Proposed edits</h3>
                <small>
                  Choose an agent response. Select the replacement wording, then
                  select the passage to replace in your draft.
                </small>
                <label>
                  Response
                  <select
                    defaultValue=""
                    onChange={(e) => {
                      setSuggestionKey(e.target.value);
                      const text =
                        proposals.find((p) => p.key === e.target.value)?.text ??
                        '';
                      setProposal(edits(text).length ? '' : text);
                    }}
                  >
                    <option value="">Choose a response</option>
                    {proposals.map((p) => (
                      <option key={p.key} value={p.key}>
                        {p.name} · {p.prompt.slice(0, 46)}
                      </option>
                    ))}
                  </select>
                </label>
                {edits(
                  proposals.find((p) => p.key === suggestionKey)?.text ?? '',
                ).map((s, i) => {
                  const at = local.indexOf(s.before);
                  const exact = at >= 0 && at === local.lastIndexOf(s.before);
                  return (
                    <section key={i}>
                      <small>Original passage</small>
                      <p>{s.before}</p>
                      <small>Proposed wording</small>
                      <p>{s.after || '(remove passage)'}</p>
                      <button
                        disabled={!exact || blocked}
                        onClick={() => {
                          setLocal(
                            local.slice(0, at) +
                              s.after +
                              local.slice(at + s.before.length),
                          );
                          setRange([0, 0]);
                        }}
                      >
                        {exact
                          ? 'Accept this edit'
                          : 'Original passage changed or ambiguous'}
                      </button>
                    </section>
                  );
                })}
                <label>
                  Replacement wording
                  <textarea
                    rows={5}
                    value={proposal}
                    onChange={(e) => setProposal(e.target.value)}
                  />
                </label>
                <button
                  disabled={!proposal.trim() || blocked}
                  onClick={() => {
                    const [a, b] = range;
                    setLocal(local.slice(0, a) + proposal + local.slice(b));
                    setRange([a + proposal.length, a + proposal.length]);
                    setProposal('');
                    editor.current?.focus();
                  }}
                >
                  {range[0] === range[1]
                    ? 'Insert wording at cursor'
                    : 'Replace selected passage'}
                </button>
              </section>
            </>
          )}
        </div>
      )}
    </aside>
  );
}
