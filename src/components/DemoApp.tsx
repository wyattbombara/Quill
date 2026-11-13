import { useEffect, useRef, useState } from 'react';
import { palettes, paletteClass, paletteStyle, defaultPalette, type PaletteName } from '../palettes';
import type { Draft } from './Composer';
import { Composer } from './Composer';
import { Transcript } from './Transcript';
import { Icon, QuillMark } from './Icon';
import type { Turn } from '../ipc';
import { UsageDialog } from './UsageDialog';
import type { CommandName } from '../slashCommands';

const sampleTurns: Turn[] = [
  { id: 1, prompt: 'How should I structure a calm writing app for long answers?', reply: 'Give the reader one clear surface, a quiet hierarchy, and enough room for the answer to breathe. The interface should feel ready before it feels busy.\n\nI would start with three anchors:\n\n1. **One reading column** with generous line length and paragraph rhythm.\n2. **A stable composer** that never moves the send control while text grows.\n3. **A soft return path** when streaming continues below the reader’s current place.\n\n```ts\n// Keep the writing surface calm\nconst focus = \'read → think → reply\';\nreturn focus;\n```\n\nThat combination makes a long answer feel like a page, not a feed.', files: ['brief.md'], model: 'gpt-4.1-mini', status: 'complete', error: null },
  { id: 2, prompt: 'Can you turn that into a small checklist?', reply: 'Here is a compact checklist to keep beside the editor:', files: [], model: 'gpt-4.1-mini', status: 'streaming', error: null },
];

export function DemoApp() {
  const query = new URLSearchParams(window.location.search);
  const [theme, setTheme] = useState<'light' | 'dark'>(() => query.get('theme') === 'dark' ? 'dark' : 'light');
  const [palette, setPalette] = useState<PaletteName>(() => { const value = query.get('palette') as PaletteName | null; return value && Object.hasOwn(palettes, value) ? value : defaultPalette; });
  const [sidebar, setSidebar] = useState(() => window.innerWidth >= 880);
  const [draft, setDraft] = useState<Draft>({ text: 'Can you turn that into a small checklist?', files: [] });
  const editor = useRef<HTMLTextAreaElement>(null);
  const [chatTitle, setChatTitle] = useState('Calm writing tools');
  const [turns, setTurns] = useState(sampleTurns);
  const [model, setModel] = useState('gpt-4.1-mini');
  const [notice, setNotice] = useState('');
  const [goal, setGoal] = useState('');
  const [usageOpen, setUsageOpen] = useState(false);
  const [systemTheme, setSystemTheme] = useState(false);
  useEffect(() => {
    if (!systemTheme) return;
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const sync = () => setTheme(media.matches ? 'dark' : 'light');
    sync();
    media.addEventListener('change', sync);
    return () => media.removeEventListener('change', sync);
  }, [systemTheme]);
  function command(name: CommandName, argument: string) {
    setNotice('');
    if (name === 'status') {
      setUsageOpen(true);
    } else if (name === 'goal') {
      if (Array.from(argument).length > 2000) return setNotice('Use a goal of 1–2,000 characters.');
      if (!argument) setNotice(goal ? 'Current goal: ' + goal : 'No goal set. Use /goal followed by what you want to achieve.');
      else {
        setGoal(argument === 'clear' ? '' : argument);
        setNotice(argument === 'clear' ? 'Goal cleared.' : 'Goal set: ' + argument);
      }
    } else if (name === 'theme') {
      if (!['light', 'dark', 'system'].includes(argument)) return setNotice('Use /theme light, /theme dark, or /theme system.');
      setSystemTheme(argument === 'system');
      if (argument === 'light' || argument === 'dark') setTheme(argument);
    } else if (name === 'model') {
      if (argument.length > 128 || /\s/.test(argument)) return setNotice('Enter a valid model ID.');
      setModel(argument);
    } else if (name === 'new') {
      setChatTitle('New chat');
      setGoal('');
      setTurns([]);
    } else if (name === 'rename') {
      if (Array.from(argument).length > 100) return setNotice('Use a title of 1–100 characters.');
      setChatTitle(argument);
    } else if (name === 'settings') {
      setNotice('Open the desktop app for Settings. In this preview, use /theme or /model to try a preference.');
    }
    setDraft((previous) => ({ ...previous, text: '' }));
  }
  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);
  return (
    <div className={'shell' + (sidebar ? '' : ' collapsed') + paletteClass(palette, theme)} style={paletteStyle(palette,theme)}>
      <aside id="sidebar" hidden={!sidebar}>
        <div className="brand"><span className="brand-icon"><QuillMark /></span>Quill</div>
        <button className="new-chat"><Icon name="plus" /> New chat</button>
        <h2 className="nav-label">Conversations</h2>
        <nav aria-label="Conversations">
          <button className="selected" aria-current="page" title={chatTitle}><Icon name="chat" /><span>{chatTitle}</span></button>
          <button title="Notes on a new idea"><Icon name="chat" /><span>Notes on a new idea</span></button>
          <button title="A very long conversation title that should truncate cleanly"><Icon name="chat" /><span>A very long conversation title that should truncate cleanly</span></button>
        </nav>
        <button className="settings-link"><Icon name="settings" /> Settings <span className="update-dot" title="Update available" /></button>
      </aside>
      <main>
        <header className="toolbar">
          <button className="icon-btn" aria-label={sidebar ? 'Hide sidebar' : 'Show sidebar'} onClick={() => setSidebar((open) => !open)}><Icon name="panel" /></button>
          <div className="heading"><strong title={chatTitle}>{chatTitle}</strong></div>
          <button className="icon-btn" aria-label="Rename"><Icon name="write" /></button>
          <button className="icon-btn danger" aria-label="Delete"><Icon name="trash" /></button>
          <button className="model-switch" aria-label={'Model settings: ' + model}><span className="model-dot" /> {model} <Icon name="chevron" /></button>
          <select className="palette-switch" aria-label="Color palette" value={palette} onChange={(event) => setPalette(event.target.value as PaletteName)}>{Object.keys(palettes).map((name) => <option key={name}>{name}</option>)}</select>
          <button className="icon-btn theme-toggle" aria-label="Toggle appearance" title="Toggle appearance" onClick={() => { setSystemTheme(false); setTheme((value) => value === 'light' ? 'dark' : 'light'); }}><Icon name={theme === 'light' ? 'moon' : 'sun'} /></button>
        </header>
        <div className="workspace">
          <Transcript chat={1} turns={turns} />
          <section className="compose-area">
            {notice && <div className="notice" role="alert"><span>{notice}</span><button aria-label="Dismiss" onClick={() => setNotice('')}>×</button></div>}
            <Composer editor={editor} draft={draft} change={setDraft} busy={false} active={false} command={command} send={() => setDraft({ text: '', files: [] })} warn={setNotice} stop={() => {}} />
          </section>
        </div>
      </main>
      {usageOpen && <UsageDialog preview close={() => setUsageOpen(false)} report={{
        chat: { requests: turns.length, reported: 0, inputTokens: 0, outputTokens: 0 },
        device: { requests: sampleTurns.length, reported: 0, inputTokens: 0, outputTokens: 0 },
      }} />}
    </div>
  );
}
