import type { CSSProperties } from 'react';
import { workspaceDefaults, type Usage } from './ipc';
import { loadDrafts, storeDrafts } from './workspaceStorage';
import { WindowBar } from './components/WindowBar';
import { useEffect, useRef, useState } from 'react';
import {
  call,
  onNewChat,
  streamChannel,
  type Chat,
  type AgentMode,
  type Turn,
  type StreamEvent,
  type Preferences,
  type UsageReport,
  type AuthStatus,
  manageUsage,
} from './ipc';
import {
  loadRoster,
  rememberRoster,
  rosterError,
  RosterPicker,
  type Roster,
} from './components/Roundtable';
import { Composer, type Draft } from './components/Composer';
import { Settings } from './components/Settings';
import { UsageDialog } from './components/UsageDialog';
import { Transcript } from './components/Transcript';
import { KeyboardGuide } from './components/KeyboardGuide';
import { ChatHistory } from './components/ChatHistory';
import { AttachmentInspector } from './components/AttachmentInspector';
import { shortModel, modelGroups } from './models';
import { encode } from './files';
import type { CommandName } from './slashCommands';
import { Icon, QuillMark } from './components/Icon';
import {
  defaultPalette,
  paletteStyle,
  palettes,
  type PaletteName,
} from './palettes';
import { DesignDirections } from './components/DesignDirections';
import { DemoApp } from './components/DemoApp';
import { PaletteLab } from './components/PaletteLab';

const blank: Draft = { text: '', files: [] };
const starters = [
  { icon: 'write', label: 'Write a draft', text: 'Help me draft ' },
  { icon: 'explain', label: 'Explain a topic', text: 'Explain this to me: ' },
  { icon: 'ideas', label: 'Explore ideas', text: 'Brainstorm ideas for ' },
] as const;
const windowsFrame = '__QUILL_WINDOWS_FRAME__' in window;
function ChatApp() {
  const [chats, setChats] = useState<Chat[]>([]);
  const [chat, setChat] = useState<number | null>(null);
  const [turns, setTurns] = useState<Turn[]>([]);
  const [drafts, setDrafts] = useState<Record<number, Draft>>({});
  const [prefs, setPrefs] = useState<Preferences>({
    model: 'gpt-4.1-mini',
    theme: 'system',
    palette: defaultPalette,
    sidebar_logo: false,
    motion: 'system',
    font: 'system',
    shortcut: 'Ctrl+Alt+Space',
  });
  const options = prefs.workspace ?? workspaceDefaults;
  const [storageReady, setStorageReady] = useState(false);
  const previousDrafts = useRef<Record<number, Draft>>({});
  const lastDraftMode = useRef<boolean | null>(null);
  const pendingOpen = useRef<number | null>(null);
  const [sideUsage, setSideUsage] = useState<Usage | null>(null);
  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false,
  );
  useEffect(() => {
    const media = window.matchMedia?.('(prefers-color-scheme: dark)');
    if (!media) return;
    const sync = () => setSystemDark(media.matches);
    sync();
    media.addEventListener('change', sync);
    return () => media.removeEventListener('change', sync);
  }, []);
  const appearance =
    prefs.theme === 'system' ? (systemDark ? 'dark' : 'light') : prefs.theme;
  const palette = prefs.palette ?? defaultPalette;
  function saveAppearance(
    change: Partial<Pick<Preferences, 'theme' | 'palette'>>,
  ) {
    void run(async () => {
      const next = { ...prefs, ...change };
      await call('save_preferences', { preferences: next });
      setPrefs(next);
    });
  }
  const [roster, setRoster] = useState<Roster | null>(null);
  const [inspectorOverride, setInspectorOverride] = useState<boolean | null>(
    null,
  );
  const [settings, setSettings] = useState(false);
  const [auth, setAuth] = useState<AuthStatus | null>(null);
  const profile = auth?.accounts.find((account) => account.id === auth.active);
  const model = auth?.active
    ? (auth.model ?? 'Choose a plan model')
    : prefs.model;
  async function syncAuth() {
    const state = await call('auth_status', {});
    setAuth(state);
    return state;
  }
  const [usage, setUsage] = useState<UsageReport | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Chat | null>(null);
  const [deleteError, setDeleteError] = useState('');
  const deleteButton = useRef<HTMLButtonElement>(null);
  const [rename, setRename] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [streamId, setStreamId] = useState<number | null>(null);
  const [notice, setNotice] = useState('');
  const [release, setRelease] = useState('');
  const [newRequests, setNewRequests] = useState(0);
  const locked = useRef(false);
  const editor = useRef<HTMLTextAreaElement>(null);
  const [sidebar, setSidebar] = useState(() => window.innerWidth > 700);
  const [narrow, setNarrow] = useState(() => window.innerWidth <= 700);
  const railButton = useRef<HTMLButtonElement>(null);
  const drawerOpen = narrow && sidebar;
  useEffect(() => {
    if (!drawerOpen) return;
    document
      .querySelector<HTMLInputElement>('#sidebar input[type="search"]')
      ?.focus();
    function dismiss(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        setSidebar(false);
        requestAnimationFrame(() => railButton.current?.focus());
      }
    }
    window.addEventListener('keydown', dismiss);
    return () => window.removeEventListener('keydown', dismiss);
  }, [drawerOpen]);
  const currentChat = chats.find((c) => c.id === chat);
  const [guide, setGuide] = useState(false);
  const [shortcutActive, setShortcutActive] = useState<boolean | null>(null);
  const title = currentChat?.title ?? 'New chat';
  const hasAttachments = turns.some((t) => t.files.length > 0);
  const inspectorOpen =
    inspectorOverride ??
    (hasAttachments && !prefs.hide_inspector && window.innerWidth > 980);
  const draft = drafts[chat ?? 0] ?? blank;

  useEffect(() => {
    const resize = () => {
      setNarrow(window.innerWidth <= 700);
      setSidebar(window.innerWidth > 700);
    };
    const query = window.matchMedia?.('(max-width:700px)');
    query?.addEventListener('change', resize);
    return () => query?.removeEventListener('change', resize);
  }, []);
  useEffect(() => {
    let live = true;
    void call('shortcut_active', {})
      .then((value) => {
        if (live) setShortcutActive(value);
      })
      .catch(() => {
        if (live) setShortcutActive(null);
      });
    return () => {
      live = false;
    };
  }, [prefs.shortcut, settings]);
  async function updateModel(value: string) {
    if (auth?.active) {
      await call('select_plan_model', { id: auth.active, model: value });
      await syncAuth();
    } else {
      const next = { ...prefs, model: value };
      await call('save_preferences', { preferences: next });
      setPrefs(next);
    }
  }
  const choices = auth?.active
    ? auth.models.map((m) => ({ id: m.slug, label: m.display_name }))
    : [...new Set([prefs.model, ...Object.values(modelGroups).flat()])].map(
        (id) => ({ id, label: id }),
      );
  function closeRail() {
    if (window.innerWidth <= 700) setSidebar(false);
  }
  async function run(action: () => Promise<void>) {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setNotice('');
    try {
      await action();
    } catch (e) {
      setNotice(String(e));
    } finally {
      locked.current = false;
      setBusy(false);
    }
  }
  async function select(id: number | null) {
    setDeleteTarget(null);
    setRoster(null);
    const next = id === null ? [] : await call('list_turns', { chat: id });
    setGuide(false);
    setInspectorOverride(null);
    setChat(id);
    setTurns(next);
    setRename(null);
  }
  async function create() {
    const c = await call('create_chat', {});
    setChats((prev) => [c, ...prev]);
    await select(c.id);
    return c;
  }
  useEffect(() => {
    void run(async () => {
      const [list, preferences] = await Promise.all([
        call('list_chats', {}),
        call('get_preferences', {}),
      ]);
      setChats(list);
      setPrefs(preferences);
      const options = preferences.workspace ?? workspaceDefaults;
      if (options.restore_drafts) {
        try {
          setDrafts(await loadDrafts(list.map((c) => c.id)));
        } catch {
          setNotice('Saved drafts could not be loaded.');
        }
      }
      const last = localStorage.getItem('quill.last-chat');
      const selected = list.find((c) => String(c.id) === last);
      await select(
        options.resume_chat && last !== 'new'
          ? (selected?.id ?? list[0]?.id ?? null)
          : null,
      );
      setStorageReady(true);
      await syncAuth();
    });
    void call('check_update', {})
      .then((update) => {
        if (update.type === 'available') setRelease(update.version);
      })
      .catch(() => {
        /* A manual check in Settings shows the network error. */
      });
    const off = onNewChat(() => setNewRequests((count) => count + 1));
    void off.catch((e) => setNotice(String(e)));
    return () => {
      void off.then(
        (dispose) => dispose(),
        () => {},
      );
    };
  }, []);
  useEffect(() => {
    if (storageReady)
      localStorage.setItem(
        'quill.last-chat',
        chat === null ? 'new' : String(chat),
      );
  }, [chat, storageReady]);
  useEffect(() => {
    if (!storageReady) return;
    if (!options.restore_drafts && lastDraftMode.current === false) return;
    lastDraftMode.current = options.restore_drafts;
    const before = previousDrafts.current;
    previousDrafts.current = options.restore_drafts ? drafts : {};
    void storeDrafts(before, drafts, options.restore_drafts).catch(() =>
      setNotice('Could not save draft changes on this device.'),
    );
  }, [drafts, options.restore_drafts, storageReady]);
  useEffect(() => {
    if (options.composer_usage === 'hidden') {
      setSideUsage(null);
      return;
    }
    let current = true,
      sequence = 0;
    const refresh = () => {
      const ticket = ++sequence;
      void call('get_usage', { chat })
        .then((report) => {
          if (current && ticket === sequence)
            setSideUsage(
              (options.composer_usage === 'today'
                ? report.today
                : report.chat) ?? null,
            );
        })
        .catch(() => {
          if (current) setSideUsage(null);
        });
    };
    setSideUsage(null);
    refresh();
    const timer = window.setInterval(refresh, busy ? 3000 : 60000);
    return () => {
      current = false;
      clearInterval(timer);
    };
  }, [options.composer_usage, chat, busy]);
  useEffect(() => {
    if (!busy && pendingOpen.current !== null && !locked.current) {
      const id = pendingOpen.current;
      pendingOpen.current = null;
      if (chats.some((c) => c.id === id)) void run(() => select(id));
    }
  }, [busy, chats]);
  useEffect(() => {
    document.documentElement.dataset.theme = prefs.theme;
  }, [prefs.theme]);
  useEffect(() => {
    const media = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    function applyMotion() {
      const preference = prefs.motion ?? 'system';
      document.documentElement.dataset.motion =
        preference === 'system'
          ? media?.matches
            ? 'reduced'
            : 'smooth'
          : preference;
    }
    applyMotion();
    media?.addEventListener('change', applyMotion);
    return () => media?.removeEventListener('change', applyMotion);
  }, [prefs.motion]);
  useEffect(() => {
    if (
      newRequests &&
      !busy &&
      !locked.current &&
      !settings &&
      !usage &&
      !deleteTarget
    ) {
      setNewRequests((n) => n - 1);
      void run(async () => {
        await create();
      });
    }
  }, [newRequests, busy, settings, usage, deleteTarget]);

  async function chooseAgents(mode: AgentMode) {
    const state = await syncAuth();
    const account = state.active;
    const limit =
      mode === 'roundtable'
        ? options.roundtable_budget
        : (state.parallel?.seats ?? 3);
    const choices = [...new Set(await call('debate_models', { account }))];
    if (!choices.length)
      throw new Error('This account has no available reply models.');
    setRoster({
      account,
      mode,
      choices,
      limit,
      capacityNote: account
        ? state.parallel?.tier === 'pro'
          ? 'Pro · ' +
            state.parallel.recent +
            ' Quill requests in the last hour · ' +
            limit +
            ' seats.'
          : 'Current account limit: ' + limit + ' seats.'
        : 'API key mode · up to 3 seats.',
      agents: loadRoster(account, choices, model, mode).slice(0, limit),
    });
    editor.current?.focus();
  }
  function command(name: CommandName, argument: string) {
    void run(async () => {
      const draftKey = chat ?? 0;
      if (name === 'debate' || name === 'parallel') {
        await chooseAgents(name === 'debate' ? 'roundtable' : 'parallel');
        setDrafts((prev) => ({
          ...prev,
          [draftKey]: { ...draft, text: argument },
        }));
        return;
      }
      if (name === 'new') {
        const next = await create();
        setDrafts((prev) => ({
          ...prev,
          [draftKey]: blank,
          [next.id]: { text: '', files: draft.files },
        }));
        editor.current?.focus();
        return;
      }
      if (name === 'status') {
        setUsage(await call('get_usage', { chat }));
      } else if (name === 'goal') {
        const goal = chats.find((item) => item.id === chat)?.goal;
        if (!argument || (argument === 'clear' && !goal)) {
          setNotice(
            goal
              ? 'Current goal: ' + goal
              : 'No goal set. Use /goal followed by what you want to achieve.',
          );
        } else {
          const updated = await call('set_chat_goal', {
            chat,
            goal: argument === 'clear' ? null : argument,
          });
          setChats((prev) =>
            chat === null
              ? [updated, ...prev]
              : prev.map((item) => (item.id === chat ? updated : item)),
          );
          setNotice(
            updated.goal ? 'Goal set: ' + updated.goal : 'Goal cleared.',
          );
          if (chat === null) {
            setChat(updated.id);
            setDrafts((prev) => ({
              ...prev,
              0: blank,
              [updated.id]: { ...draft, text: '' },
            }));
            return;
          }
        }
      } else if (name === 'rename') {
        if (chat === null) throw new Error('Start a chat before renaming it.');
        await call('rename_chat', { chat, title: argument });
        setChats((prev) =>
          prev.map((item) =>
            item.id === chat ? { ...item, title: argument } : item,
          ),
        );
        setRename(null);
      } else if (name === 'model') {
        await updateModel(argument);
      } else if (name === 'theme') {
        const next = { ...prefs };
        if (
          argument === 'light' ||
          argument === 'dark' ||
          argument === 'system'
        )
          next.theme = argument;
        else
          throw new Error('Use /theme light, /theme dark, or /theme system.');
        await call('save_preferences', { preferences: next });
        setPrefs(next);
      } else if (name === 'settings') {
        setSettings(true);
      }
      setDrafts((prev) => ({ ...prev, [draftKey]: { ...draft, text: '' } }));
    });
  }
  function send(prompt: string) {
    if (!prompt.trim() && !draft.files.length) return;
    void run(async () => {
      if (roster && roster.agents.length > roster.limit)
        throw new Error('Remove agents to fit the current limit.');
      if (roster && rosterError(roster.agents))
        throw new Error(rosterError(roster.agents));
      setGuide(false);
      const files = await Promise.all(draft.files.map(encode));
      const id = chat ?? (await create()).id;
      // Move an unsent first draft to its newly allocated conversation.
      if (chat === null)
        setDrafts((prev) => ({ ...prev, 0: blank, [id]: draft }));
      let live: number | null = null;
      let active = true;
      let pending: StreamEvent[] = [];
      let paint = 0;
      function flush() {
        paint = 0;
        const batch = pending;
        pending = [];
        if (!batch.length) return;
        setTurns((prev) =>
          prev.map((turn) => {
            if (turn.id !== live) return turn;
            let reply = turn.reply;
            let replyModel = turn.model;
            const entries = turn.debate?.entries.slice();
            for (const ev of batch) {
              if (ev.type === 'delta') reply += ev.text;
              else if (ev.type === 'model') {
                if (!ev.agent) replyModel = ev.model;
                else if (entries) {
                  const index = entries.findIndex(
                    (entry) => entry.id === ev.id,
                  );
                  if (index >= 0)
                    entries[index] = { ...entries[index], model: ev.model };
                }
              } else if (entries && ev.type === 'agentStart')
                entries.push(ev.entry);
              else if (
                entries &&
                (ev.type === 'agentDelta' || ev.type === 'agentEnd')
              ) {
                const i = entries.findIndex((e) => e.id === ev.id);
                if (i >= 0)
                  entries[i] =
                    ev.type === 'agentDelta'
                      ? { ...entries[i], text: entries[i].text + ev.text }
                      : { ...entries[i], status: ev.status };
              }
            }
            return {
              ...turn,
              model: replyModel,
              reply,
              ...(turn.debate && entries
                ? { debate: { ...turn.debate, entries } }
                : {}),
            };
          }),
        );
      }
      const channel = streamChannel((event) => {
        if (!active) return;
        if (event.type === 'started') {
          live = event.id;
          if (roster) rememberRoster(roster);
          setStreamId(event.id);
          setRoster(roster?.mode === 'parallel' ? roster : null);
          setDrafts((prev) => ({ ...prev, [id]: blank }));
          setTurns((prev) => [
            ...prev,
            {
              id: event.id,
              ...(roster
                ? {
                    debate: {
                      mode: roster.mode,
                      agents: roster.agents,
                      entries: [],
                    },
                  }
                : {}),
              prompt,
              reply: '',
              files: draft.files.map((f) => f.name),
              model: event.model || model,
              status: 'streaming',
              error: null,
            },
          ]);
        } else {
          pending.push(event);
          if (!paint) paint = requestAnimationFrame(flush);
        }
      });
      try {
        await call('send_message', {
          ...(roster
            ? {
                debate: {
                  mode: roster.mode,
                  account: roster.account,
                  agents: roster.agents,
                },
              }
            : {}),
          chat: id,
          prompt,
          files,
          channel,
        });
        setDrafts((prev) => ({ ...prev, [id]: blank }));
      } catch (error) {
        // A new conversation gets its ID before authorization. Preserve the reviewed
        // lineup on a startup failure so a retry still uses Roundtable.
        if (roster && live === null) setRoster(roster);
        throw error;
      } finally {
        active = false;
        cancelAnimationFrame(paint);
        flush();
        setStreamId(null);
        const [list, history] = await Promise.all([
          call('list_chats', {}),
          call('list_turns', { chat: id }),
        ]);
        setChats(list);
        setTurns(history);
        if (roster?.mode === 'parallel') {
          const state = await syncAuth();
          setRoster((prev) =>
            prev && prev.account === state.active && prev.mode === 'parallel'
              ? {
                  ...prev,
                  limit: state.parallel?.seats ?? 3,
                  capacityNote:
                    'Current account limit: ' +
                    (state.parallel?.seats ?? 3) +
                    ' seats.',
                }
              : prev,
          );
        }
        const finished = history.find(
          (t) => t.id === live && t.status === 'complete',
        );
        if (
          finished &&
          options.reply_notifications &&
          (!document.hasFocus() || document.hidden) &&
          typeof Notification !== 'undefined' &&
          Notification.permission === 'granted'
        ) {
          try {
            const notification = new Notification(
              roster ? 'Quill · Agent run complete' : 'Quill · Reply ready',
              {
                body: options.preview_notifications
                  ? finished.reply.slice(0, 160)
                  : 'Open the conversation in Quill.',
                tag: 'quill-chat-' + id,
                silent: true,
              },
            );
            notification.onclick = () => {
              notification.close();
              void call('show_chat_window', {}).catch((e) =>
                setNotice(String(e)),
              );
              if (locked.current) pendingOpen.current = id;
              else
                void run(async () => {
                  const chats = await call('list_chats', {});
                  if (chats.some((c) => c.id === id)) await select(id);
                });
            };
          } catch {
            setNotice(
              'The reply is ready. The system could not display its notification.',
            );
          }
        }
      }
    });
  }
  function keepChat() {
    if (locked.current) return;
    setDeleteTarget(null);
    deleteButton.current?.focus();
  }
  function confirmDelete() {
    if (!deleteTarget || deleteTarget.id !== chat) return;
    const id = deleteTarget.id;
    void run(async () => {
      setDeleteError('');
      try {
        await call('delete_chat', { chat: id });
      } catch (error) {
        setDeleteError(String(error));
        return;
      }
      setDeleteTarget(null);
      const remaining = chats.filter((item) => item.id !== id);
      setChats(remaining);
      setDrafts((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      setChat(null);
      setTurns([]);
      setGuide(false);
      await select(remaining[0]?.id ?? null);
      requestAnimationFrame(() => editor.current?.focus());
    });
  }
  return (
    <div
      data-font={prefs.font ?? 'system'}
      data-windows-frame={windowsFrame || undefined}
      data-density={prefs.compact ? 'compact' : 'comfortable'}
      data-width={prefs.wide_reading ? 'wide' : 'standard'}
      className={
        'shell live-workspace afterglow' +
        (sidebar ? '' : ' collapsed') +
        (appearance === 'dark' ? ' dark-mode' : '')
      }
      style={
        {
          ...paletteStyle(palette, appearance),
          '--reply-size': options.reply_size + 'px',
          '--code-size': options.code_size + 'px',
        } as CSSProperties
      }
    >
      {windowsFrame && <WindowBar />}
      <aside id="sidebar" aria-hidden={!sidebar} inert={!sidebar}>
        {prefs.sidebar_logo && (
          <div className="brand">
            <span className="brand-icon">
              <QuillMark />
            </span>
            Quill
          </div>
        )}
        <div className="chats-head">
          <strong>Chats</strong>
          <button
            aria-label="New chat"
            title="New chat"
            className="new-chat"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                await create();
                closeRail();
              })
            }
          >
            <Icon name="write" />
          </button>
        </div>
        <ChatHistory
          chats={chats}
          active={chat}
          busy={busy}
          select={(id) => {
            closeRail();
            void run(() => select(id));
          }}
        />
        <div className="rail-footer">
          <button
            className="settings-link"
            disabled={busy}
            onClick={() => {
              closeRail();
              setSettings(true);
            }}
          >
            <Icon name="settings" />
            Settings
            {release && (
              <span
                className="update-dot"
                title={'Update available: ' + release}
              />
            )}
          </button>
          <button
            className="rail-usage"
            disabled={busy}
            onClick={() => {
              closeRail();
              void run(async () => setUsage(await call('get_usage', { chat })));
            }}
          >
            <Icon name="usage" />
            Usage
          </button>
        </div>
      </aside>
      {narrow && (
        <button
          className="rail-scrim"
          aria-hidden={!drawerOpen}
          inert={!drawerOpen}
          tabIndex={-1}
          aria-label="Close sidebar"
          onClick={() => setSidebar(false)}
        />
      )}
      <main inert={drawerOpen}>
        <header className="toolbar">
          <button
            className="icon-btn"
            aria-label={sidebar ? 'Hide sidebar' : 'Show sidebar'}
            title={sidebar ? 'Hide sidebar' : 'Show sidebar'}
            ref={railButton}
            aria-controls="sidebar"
            aria-expanded={sidebar}
            onClick={() => setSidebar((open) => !open)}
          >
            <Icon name="panel" />
          </button>
          <span className="crumb-prefix">
            Chats <b>›</b>
          </span>
          <div className="heading">
            {rename !== null ? (
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  if (chat !== null)
                    void run(async () => {
                      await call('rename_chat', { chat, title: rename });
                      setRename(null);
                      setChats(await call('list_chats', {}));
                    });
                }}
              >
                <input
                  aria-label="Conversation title"
                  autoFocus
                  maxLength={100}
                  value={rename}
                  onChange={(e) => setRename(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Escape') setRename(null);
                  }}
                />
                <button disabled={busy}>Save</button>
                <button type="button" onClick={() => setRename(null)}>
                  Cancel
                </button>
              </form>
            ) : (
              <strong title={title}>{title}</strong>
            )}
          </div>
          {chat !== null && rename === null && (
            <div className="row">
              <button
                className="icon-btn"
                title="Rename"
                aria-label="Rename"
                disabled={busy}
                onClick={() => setRename(title)}
              >
                <Icon name="write" />
              </button>
              <button
                className="icon-btn danger"
                title="Delete"
                aria-label="Delete"
                disabled={busy}
                ref={deleteButton}
                aria-expanded={!!deleteTarget}
                aria-controls={deleteTarget ? 'delete-review' : undefined}
                onClick={() => {
                  setDeleteError('');
                  setDeleteTarget(deleteTarget ? null : (currentChat ?? null));
                }}
              >
                <Icon name="trash" />
              </button>
            </div>
          )}
          <button
            className="icon-btn guide-toggle"
            title="Keyboard guide"
            aria-label="Keyboard guide"
            aria-pressed={guide}
            disabled={busy}
            onClick={() => setGuide(!guide)}
          >
            <Icon name="keyboard" />
          </button>
          <label className="model-switch" title="Choose reply model">
            <span className="model-dot" />
            <span>
              {shortModel(choices.find((c) => c.id === model)?.label ?? model)}
            </span>
            <Icon name="chevron" />
            <select
              aria-label="Reply model"
              value={model}
              disabled={busy}
              onChange={(e) => {
                const id = e.target.value;
                if (id === ':settings') setSettings(true);
                else void run(() => updateModel(id));
              }}
            >
              {!choices.some((c) => c.id === model) && (
                <option value={model}>{model}</option>
              )}
              {choices.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.label}
                </option>
              ))}
              <option value=":settings">Model settings…</option>
            </select>
          </label>
          <select
            className="palette-switch"
            aria-label="Color palette"
            disabled={busy}
            value={palette}
            onChange={(e) =>
              saveAppearance({ palette: e.target.value as PaletteName })
            }
          >
            {Object.keys(palettes).map((name) => (
              <option key={name}>{name}</option>
            ))}
          </select>
          <button
            className="icon-btn theme-toggle"
            aria-label="Toggle appearance"
            title="Toggle appearance"
            disabled={busy}
            onClick={() =>
              saveAppearance({
                theme: appearance === 'dark' ? 'light' : 'dark',
              })
            }
          >
            <Icon name={appearance === 'light' ? 'moon' : 'sun'} />
          </button>
          <button
            className="icon-btn inspector-toggle"
            aria-label="Attachments"
            title="Attachments"
            aria-expanded={inspectorOpen}
            onClick={() => setInspectorOverride(!inspectorOpen)}
          >
            <Icon name="attach" />
          </button>
        </header>
        {deleteTarget && (
          <section
            id="delete-review"
            className="delete-review"
            aria-labelledby="delete-question"
            onBlur={(e) => {
              if (
                !locked.current &&
                e.relatedTarget &&
                e.relatedTarget !== deleteButton.current &&
                !e.currentTarget.contains(e.relatedTarget)
              )
                setDeleteTarget(null);
            }}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.preventDefault();
                keepChat();
              }
            }}
          >
            <Icon name="trash" />
            <div className="delete-copy">
              <strong id="delete-question">
                Delete “{deleteTarget.title}”?
              </strong>
              <span>This removes the chat and its attachments from Quill. Copies may remain in backups or on disk. Secure erasure is not guaranteed.</span>
              {deleteError && (
                <span className="delete-error" role="alert">
                  {deleteError}
                </span>
              )}
            </div>
            <div className="delete-actions">
              <button autoFocus disabled={busy} onClick={keepChat}>
                Keep chat
              </button>
              <button
                className="confirm-delete"
                disabled={busy}
                onClick={confirmDelete}
              >
                {busy ? 'Deleting…' : 'Delete chat'}
              </button>
            </div>
          </section>
        )}
        <div className="desktop-body">
          <div className="workspace">
            {turns.length && !guide ? (
              <Transcript
                key={chat}
                chat={chat}
                turns={turns}
                title={title}
                followReplies={options.follow_replies}
              />
            ) : (
              <div className="guide-reader" key={chat}>
                <div className="reading">
                  {guide ? (
                    <KeyboardGuide
                      shortcut={prefs.shortcut}
                      available={shortcutActive}
                      modifierSend={prefs.modifier_send}
                    />
                  ) : (
                    <h1 className="thread-title">{title}</h1>
                  )}
                  {guide && (
                    <button onClick={() => setGuide(false)}>
                      Back to conversation
                    </button>
                  )}
                  {!turns.length && !guide && (
                    <div className="starters">
                      {starters.map((item) => (
                        <button
                          key={item.icon}
                          disabled={busy}
                          onClick={() => {
                            setDrafts((prev) => ({
                              ...prev,
                              [chat ?? 0]: { ...draft, text: item.text },
                            }));
                            editor.current?.focus();
                          }}
                        >
                          <Icon name={item.icon} />
                          {item.label}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            )}
            <section className="compose-area">
              {currentChat?.goal && (
                <div className="current-goal">
                  <Icon name="goal" /> <span>Goal · {currentChat.goal}</span>
                </div>
              )}
              {notice && (
                <div role="alert" className="notice">
                  <span>{notice}</span>
                  <button aria-label="Dismiss" onClick={() => setNotice('')}>
                    ×
                  </button>
                </div>
              )}
              {roster && !busy && (
                <RosterPicker
                  key={roster.mode + roster.account}
                  roster={roster}
                  busy={busy}
                  update={(agents) => setRoster({ ...roster, agents })}
                />
              )}
              <Composer
                roundtable={{
                  mode: roster?.mode,
                  ready:
                    !!roster &&
                    roster.agents.length <= roster.limit &&
                    !rosterError(roster.agents),
                  toggle: (mode) => {
                    if (roster?.mode === mode) setRoster(null);
                    else void run(() => chooseAgents(mode));
                  },
                }}
                modifierSend={prefs.modifier_send}
                editor={editor}
                draft={draft}
                change={(value) =>
                  setDrafts((prev) => ({ ...prev, [chat ?? 0]: value }))
                }
                busy={busy}
                active={streamId !== null}
                send={send}
                command={command}
                warn={setNotice}
                stop={() => {
                  if (streamId !== null)
                    void call('stop_reply', { id: streamId }).catch((e) =>
                      setNotice(String(e)),
                    );
                }}
              />
              <div className="plan-indicator">
                {sideUsage && options.composer_usage !== 'hidden' && (
                  <button
                    className="usage-counter"
                    onClick={() =>
                      void run(async () =>
                        setUsage(await call('get_usage', { chat })),
                      )
                    }
                  >
                    {options.composer_usage === 'today' ? 'Today' : 'This chat'}{' '}
                    · {sideUsage.requests}{' '}
                    {sideUsage.requests === 1 ? 'request' : 'requests'}
                    {sideUsage.reported > 0
                      ? ' · ' +
                        (
                          sideUsage.inputTokens + sideUsage.outputTokens
                        ).toLocaleString() +
                        ' tokens'
                      : ''}
                  </button>
                )}
                <span>
                  <i
                    className={
                      profile?.permitted && profile.connected ? 'connected' : ''
                    }
                  />
                  {auth?.active
                    ? profile?.permitted && profile.connected
                      ? 'Using ChatGPT plan'
                      : 'ChatGPT plan disconnected'
                    : 'API key mode'}
                </span>
                {auth?.active && (
                  <button
                    onClick={() =>
                      void manageUsage().catch((e) => setNotice(String(e)))
                    }
                  >
                    Manage usage
                  </button>
                )}
              </div>
            </section>
          </div>
          {inspectorOpen && (
            <AttachmentInspector
              key={chat ?? 'empty'}
              chat={chat}
              turns={turns}
              close={() => setInspectorOverride(false)}
            />
          )}
        </div>
      </main>
      {usage && (
        <UsageDialog
          report={usage}
          plan={Boolean(auth?.active)}
          close={() => setUsage(null)}
        />
      )}
      {settings && (
        <Settings
          auth={auth}
          syncAuth={syncAuth}
          preferences={prefs}
          saved={(next) => {
            if (next.hide_inspector !== prefs.hide_inspector)
              setInspectorOverride(null);
            setPrefs(next);
            if (roster?.mode === 'roundtable')
              setRoster({
                ...roster,
                limit: (next.workspace ?? workspaceDefaults).roundtable_budget,
              });
          }}
          close={() => {
            setSettings(false);
            requestAnimationFrame(() => editor.current?.focus());
          }}
        />
      )}
    </div>
  );
}

export function App() {
  const params = new URLSearchParams(window.location.search);
  if (params.has('directions')) {
    return <DesignDirections />;
  }
  if (params.has('palettes')) return <PaletteLab />;
  if (params.has('demo')) return <DemoApp />;
  return <ChatApp />;
}
