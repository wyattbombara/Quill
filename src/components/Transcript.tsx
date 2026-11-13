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
function ParallelReplies({ turn }: { turn: Turn }) {
  const team = turn.debate!;
  return (
    <section className="parallel-replies" aria-label="Parallel replies">
      <h2>Parallel replies</h2>
      {team.agents.map((agent, seat) => {
        const entry = team.entries.find((e) => e.seat === seat);
        const status =
          entry?.status ??
          (turn.status === 'streaming' ? 'queued' : turn.status);
        return (
          <section
            key={seat}
            className={'parallel-reply seat-' + seat}
            aria-label={agent.name + ' reply'}
          >
            <header>
              <strong>{agent.name}</strong>
              <small title={entry?.model ?? agent.model}>
                {shortModel(entry?.model ?? agent.model)}
              </small>
              <span>{status}</span>
            </header>
            {agent.focus && <p className="agent-focus">{agent.focus}</p>}
            {entry?.text && (
              <div className="markdown">
                <Suspense
                  fallback={<p className="plain-reply">{entry.text}</p>}
                >
                  <MarkdownBody text={entry.text} />
                </Suspense>
              </div>
            )}
          </section>
        );
      })}
    </section>
  );
}

const Entry = memo(function Entry({ turn }: { turn: Turn }) {
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
    <article className="turn">
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
      {parallel && <ParallelReplies turn={turn} />}
      {turn.debate && !parallel && (
        <Roundtable debate={turn.debate} status={turn.status} />
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
              <MarkdownBody text={turn.reply} />
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
  followReplies = true,
  title,
}: {
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
            <Entry key={t.id} turn={t} />
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
