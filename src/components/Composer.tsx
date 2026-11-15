import {
  type RefObject,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import type { AgentMode } from '../ipc';
import { commands, parseInput, type CommandName } from '../slashCommands';
import { accepts, validateFiles } from '../files';
import { Icon } from './Icon';

export interface Draft {
  text: string;
  files: File[];
}
interface Props {
  modifierSend?: boolean;
  editor: RefObject<HTMLTextAreaElement | null>;
  draft: Draft;
  change: (draft: Draft) => void;
  busy: boolean;
  active: boolean;
  roundtable?: {
    mode?: AgentMode;
    ready: boolean;
    toggle: (mode: AgentMode) => void;
  };
  stop: () => void;
  send: (text: string) => void;
  command: (name: CommandName, argument: string) => void;
  warn: (message: string) => void;
}
export function Composer({
  editor,
  draft,
  change,
  busy,
  active,
  stop,
  send,
  command,
  warn,
  roundtable,
  modifierSend = false,
}: Props) {
  const input = useRef<HTMLInputElement>(null);
  const [drag, setDrag] = useState(false);
  const [selected, setSelected] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const menuId = useId();
  const matches =
    !busy && !dismissed && /^\/[a-z]*$/i.test(draft.text)
      ? commands.filter((item) =>
          ('/' + item.name).startsWith(
            draft.text.toLowerCase().replace(/[ \t]+/g, ' '),
          ),
        )
      : [];
  const index = Math.min(selected, matches.length - 1);
  const optionId = (i: number) => menuId + '-' + i;
  useLayoutEffect(() => {
    document
      .getElementById(menuId + '-' + index)
      ?.scrollIntoView?.({ block: 'nearest' });
  }, [index, menuId, draft.text]);
  function edit(text: string) {
    setDismissed(false);
    setSelected(0);
    change({ ...draft, text });
  }
  function submit(text: string) {
    if (busy) return;
    if (text === '/') {
      setDismissed(false);
      editor.current?.focus();
      return;
    }
    const input = parseInput(text);
    if (input.kind === 'message') return send(input.text);
    if (input.kind === 'error') return warn(input.message);
    if (input.name === 'help') {
      warn('');
      edit('/');
      editor.current?.focus();
    } else {
      command(input.name, input.argument);
    }
  }
  function choose(item: (typeof commands)[number], completeOnly = false) {
    const token = '/' + item.name;
    if (
      !completeOnly &&
      (item.name === 'roundtable' ||
        item.name === 'parallel' ||
        item.name === 'goal') &&
      draft.text
        .toLowerCase()
        .replace(/[ \t]+/g, ' ')
        .trimEnd() === token
    ) {
      submit(token);
      return;
    }
    if (completeOnly || item.usage !== token) {
      edit(token + (item.usage === token ? '' : ' '));
      editor.current?.focus();
    } else {
      submit(token);
    }
  }
  useLayoutEffect(() => {
    const field = editor.current;
    if (!field) return;
    function fit() {
      if (!field) return;
      field.style.height = 'auto';
      field.style.height = field.scrollHeight + 'px';
    }
    fit();
    window.addEventListener('resize', fit);
    return () => window.removeEventListener('resize', fit);
  }, [draft.text, editor]);
  const add = (incoming: File[]) => {
    if (busy) return;
    const files = [...draft.files, ...incoming];
    try {
      validateFiles(files);
      change({ ...draft, files });
    } catch (error) {
      warn(String(error));
    }
  };
  return (
    <>
      {roundtable?.mode && (
        <div className="mode-strip" aria-label="Active agent session">
          <span>
            {roundtable.mode === 'parallel' ? 'Parallel agents' : 'Roundtable'}
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={() =>
              roundtable.mode && roundtable.toggle(roundtable.mode)
            }
          >
            Return to chat
          </button>
        </div>
      )}
      <form
        className={'composer' + (drag ? ' dragging' : '')}
        onSubmit={(e) => {
          e.preventDefault();
          if (matches[index]) choose(matches[index]);
          else submit(draft.text);
        }}
        onDragOver={(e) => {
          e.preventDefault();
          setDrag(true);
        }}
        onDragLeave={() => setDrag(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDrag(false);
          add(Array.from(e.dataTransfer.files));
        }}
      >
        {matches.length > 0 && (
          <div className="command-menu">
            <div
              className="command-scroll"
              id={menuId}
              role="listbox"
              aria-label="Slash commands"
            >
              {matches.map((item, i) => (
                <button
                  key={item.name}
                  type="button"
                  id={optionId(i)}
                  role="option"
                  tabIndex={-1}
                  aria-selected={i === index}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => choose(item)}
                >
                  <strong>{item.usage}</strong>
                  <span>{item.description}</span>
                </button>
              ))}
            </div>
          </div>
        )}
        {draft.files.length > 0 && (
          <div className="files">
            {draft.files.map((file, i) => (
              <button
                type="button"
                className="file-chip"
                key={i}
                disabled={busy}
                title={'Remove ' + file.name}
                aria-label={'Remove attachment ' + file.name}
                onClick={() =>
                  change({
                    ...draft,
                    files: draft.files.filter((_, j) => j !== i),
                  })
                }
              >
                <span>{file.name}</span>
                <b aria-hidden="true">×</b>
              </button>
            ))}
          </div>
        )}
        <textarea
          ref={editor}
          aria-label="Message"
          placeholder={
            drag
              ? 'Drop your files here'
              : roundtable?.mode
                ? roundtable.mode === 'parallel'
                  ? 'Message all agents…'
                  : 'Ask the roundtable…'
                : 'Message Quill…'
          }
          aria-controls={matches.length ? menuId : undefined}
          aria-activedescendant={matches.length ? optionId(index) : undefined}
          aria-autocomplete="list"
          rows={1}
          readOnly={busy}
          value={draft.text}
          onChange={(e) => edit(e.target.value)}
          onKeyDown={(e) => {
            if (
              busy ||
              e.nativeEvent.isComposing ||
              e.nativeEvent.keyCode === 229 ||
              e.shiftKey ||
              ((e.ctrlKey || e.metaKey) &&
                (!modifierSend || e.key !== 'Enter')) ||
              e.altKey
            )
              return;
            if (
              matches.length &&
              (e.key === 'ArrowDown' || e.key === 'ArrowUp')
            ) {
              e.preventDefault();
              setSelected(
                (index + (e.key === 'ArrowDown' ? 1 : -1) + matches.length) %
                  matches.length,
              );
            } else if (matches.length && e.key === 'Escape') {
              e.preventDefault();
              setDismissed(true);
            } else if (
              matches.length &&
              e.key === 'Tab' &&
              draft.text !== '/' + matches[index].name
            ) {
              e.preventDefault();
              choose(matches[index], true);
            } else if (
              e.key === 'Enter' &&
              (matches.length || !modifierSend || e.ctrlKey || e.metaKey)
            ) {
              e.preventDefault();
              if (matches[index]) choose(matches[index]);
              else submit(draft.text);
            }
          }}
        />
        <div className="composer-actions">
          <input
            ref={input}
            type="file"
            hidden
            multiple
            accept={accepts}
            onChange={(e) => {
              add(Array.from(e.target.files ?? []));
              e.target.value = '';
            }}
          />
          <button
            type="button"
            className="attach"
            aria-label="Attach"
            title="Attach a file"
            disabled={busy}
            onClick={() => input.current?.click()}
          >
            <Icon name="attach" />
          </button>
          <button
            type="button"
            className="slash-trigger"
            aria-label="Browse slash commands"
            title="Browse slash commands"
            disabled={busy}
            onClick={() => {
              if (
                draft.text &&
                !/^\/[a-z]*$/i.test(draft.text)
              ) {
                warn('Start a message with / to browse commands.');
                editor.current?.focus();
                return;
              }
              edit('/');
              editor.current?.focus();
            }}
          >
            /
          </button>
          <span className="command-hint">Type / for commands</span>
          {busy && active ? (
            <button type="button" className="primary" onClick={stop}>
              <Icon name="stop" /> Stop
            </button>
          ) : (
            <button
              type="submit"
              className={roundtable?.mode ? 'primary start-debate' : 'primary'}
              disabled={
                busy ||
                (roundtable?.mode && !roundtable.ready) ||
                (!draft.text.trim() && !draft.files.length)
              }
            >
              {busy
                ? 'Working…'
                : matches.length || parseInput(draft.text).kind !== 'message'
                  ? 'Run'
                  : roundtable?.mode
                    ? roundtable.mode === 'parallel'
                      ? 'Run agents'
                      : 'Start debate'
                    : 'Send'}{' '}
              <Icon name="send" />
            </button>
          )}
        </div>
      </form>
    </>
  );
}
