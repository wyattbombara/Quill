import {
  lazy,
  memo,
  Suspense,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { shortModel } from '../models';
import type { Turn } from '../ipc';
import { Roundtable } from './Roundtable';
import { Icon, QuillMark } from './Icon';

const MarkdownBody = lazy(() => import('./MarkdownBody'));
export interface TurnActions {
  source: (chat: number, turn: number) => void;
  busy: boolean;
  vote: (turn: number, seat: number) => void;
  fork: (turn: number) => void;
  pin: (turn: Turn) => void;
}
function ParallelReplies({
  turn,
  actions,
}: {
  turn: Turn;
  actions?: TurnActions;
}) {
  const team = turn.debate!;
  const ballot = team.ballot;
  const blind = !!ballot && ballot.winner === null;
  return (
    <section className="parallel-replies" aria-label="Parallel replies">
      <h2>Parallel replies</h2>
      {ballot && (
        <p>
          {blind
            ? 'Pick the answer you would use to reveal the models.'
            : 'Models revealed · your choice is marked below.'}
        </p>
      )}
      {(ballot?.ordering ?? team.agents.map((_, i) => i)).map(
        (seat, position) => {
          const agent = team.agents[seat];
          const label = blind
            ? 'Answer ' + String.fromCharCode(65 + position)
            : agent.name;
          const entry = team.entries.find((e) => e.seat === seat);
          let answer = entry?.text ?? '';
          if (blind) {
            const identities = [
              ...team.agents.flatMap((a) => [a.name, a.model]),
              ...team.entries.map((e) => e.model ?? ''),
            ]
              .filter(Boolean)
              .sort((a, b) => b.length - a.length);
            for (const id of identities)
              answer = answer.replaceAll(id, '[hidden]');
          }
          const status =
            entry?.status ??
            (turn.status === 'streaming' ? 'queued' : turn.status);
          return (
            <section
              key={seat}
              className={'parallel-reply seat-' + (blind ? position : seat)}
              aria-label={label + ' reply'}
            >
              <header>
                <strong>{label}</strong>
                {!blind && (
                  <small title={entry?.model ?? agent.model}>
                    {shortModel(entry?.model ?? agent.model)}
                  </small>
                )}
                <span>{status}</span>
              </header>
              {ballot?.winner === seat && <strong>Your pick</strong>}
              {blind && actions && entry?.status === 'complete' && (
                <button
                  disabled={actions.busy || turn.status === 'streaming'}
                  onClick={() => actions.vote(turn.id, seat)}
                >
                  Choose {label}
                </button>
              )}
              {!blind && agent.focus && (
                <p className="agent-focus">{agent.focus}</p>
              )}
              {answer && (
                <div className="markdown">
                  <Suspense fallback={<p className="plain-reply">{answer}</p>}>
                    <MarkdownBody text={answer} source={actions?.source} />
                  </Suspense>
                </div>
              )}
            </section>
          );
        },
      )}
    </section>
  );
}

const Entry = memo(function Entry({
  turn,
  actions,
}: {
  turn: Turn;
  actions?: TurnActions;
}) {
  const [copyStatus, setCopyStatus] = useState('');
  async function copyReply() {
    try {
      await navigator.clipboard.writeText(turn.reply);
      setCopyStatus('Copied');
    } catch {
      setCopyStatus('Select the reply and copy it manually.');
    }
  }
  const parallel = turn.debate?.mode === 'parallel';
  const chairEntry = turn.debate?.entries
    .filter((e) => e.phase === 'conclusion' || e.phase === 'review')
    .at(-1);
  const replyModel =
    chairEntry?.model ?? turn.debate?.agents.at(-1)?.model ?? turn.model;
  return (
    <article className="turn" id={'turn-' + turn.id} tabIndex={-1}>
      <div className="user-block">
        <div className="speaker">You</div>
        <div className="user-bubble">
          {turn.prompt && <div className="user-message">{turn.prompt}</div>}
          {!!turn.files.length && (
            <div className="files">
              {turn.files.map((name, i) => (
                <span key={i}>
                  <Icon name="attach" />
                  {name}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
      {parallel && <ParallelReplies turn={turn} actions={actions} />}
      {turn.debate && !parallel && (
        <Roundtable
          debate={turn.debate}
          status={turn.status}
          turnId={turn.id}
        />
      )}
      {!parallel && (!turn.debate || turn.reply) && (
        <div className="speaker assistant">
          <span className="reply-mark">
            <QuillMark />
          </span>
          {turn.debate ? 'Roundtable answer' : 'Quill'}{' '}
          <small title={replyModel}>{shortModel(replyModel)}</small>
        </div>
      )}
      {!parallel && (
        <div className="markdown">
          {turn.reply ? (
            <Suspense fallback={<p className="plain-reply">{turn.reply}</p>}>
              <MarkdownBody text={turn.reply} source={actions?.source} />
            </Suspense>
          ) : (
            turn.status === 'streaming' &&
            !turn.debate && (
              <div
                className="typing"
                role="status"
                aria-label="Waiting for reply"
              >
                <i />
                <i />
                <i />
              </div>
            )
          )}
        </div>
      )}
      {!parallel && turn.status === 'complete' && turn.reply && (
        <div className="reply-tools">
          <button
            aria-label="Copy reply"
            title="Copy reply"
            onClick={copyReply}
          >
            <Icon name="copy" />
          </button>
          <small role="status">{copyStatus}</small>
        </div>
      )}
      {actions && turn.status !== 'streaming' && (
        <div className="studio-actions">
          <button disabled={actions.busy} onClick={() => actions.fork(turn.id)}>
            Try both · branch here
          </button>
          <button disabled={actions.busy} onClick={() => actions.pin(turn)}>
            Pin to project
          </button>
        </div>
      )}
      {turn.status !== 'complete' && turn.status !== 'streaming' && (
        <p className="status">
          {turn.error ||
            (turn.status === 'stopped'
              ? 'Reply stopped.'
              : 'Reply interrupted.')}
        </p>
      )}
    </article>
  );
});

export function Transcript({
  turns,
  chat,
  activity,
  actions,
  followReplies = true,
  title,
}: {
  actions?: TurnActions;
  followReplies?: boolean;
  activity?: number;
  title?: string;
  turns: Turn[];
  chat: number | null;
}) {
  const pane = useRef<HTMLDivElement>(null);
  const reading = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const [away, setAway] = useState(false);
  function jump() {
    if (pane.current) pane.current.scrollTop = pane.current.scrollHeight;
  }
  useLayoutEffect(() => {
    follow.current = true;
    setAway(false);
  }, [chat]);
  useLayoutEffect(() => {
    if (followReplies && follow.current) jump();
    else if (pane.current)
      setAway(
        pane.current.scrollHeight -
          pane.current.clientHeight -
          pane.current.scrollTop >
          60,
      );
  }, [turns, followReplies]);
  useEffect(() => {
    if (!reading.current || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => {
      if (followReplies && follow.current) jump();
      else if (pane.current)
        setAway(
          pane.current.scrollHeight -
            pane.current.clientHeight -
            pane.current.scrollTop >
            60,
        );
    });
    observer.observe(reading.current);
    return () => observer.disconnect();
  }, [followReplies]);
  return (
    <div className="thread-view">
      <div
        className="transcript"
        ref={pane}
        tabIndex={0}
        role="region"
        aria-label="Messages"
        onScroll={(e) => {
          const el = e.currentTarget;
          follow.current =
            el.scrollHeight - el.scrollTop - el.clientHeight < 60;
          setAway(!follow.current);
        }}
      >
        <div className="reading" ref={reading}>
          {title && <h1 className="thread-title">{title}</h1>}
          {Boolean(activity) && (
            <div className="thread-date">
              Last active ·{' '}
              {new Date(activity! * 1000).toLocaleDateString(undefined, {
                dateStyle: 'medium',
              })}
            </div>
          )}
          {turns.map((t) => (
            <Entry key={t.id} turn={t} actions={actions} />
          ))}
        </div>
      </div>
      {away && (
        <button
          className="jump"
          onClick={() => {
            follow.current = true;
            setAway(false);
            jump();
          }}
        >
          <Icon name="down" /> Jump to latest
        </button>
      )}
    </div>
  );
}
