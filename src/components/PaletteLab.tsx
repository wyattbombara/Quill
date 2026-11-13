import { useState } from 'react';
import type { CSSProperties } from 'react';
import { Icon, QuillMark } from './Icon';

type Palette = { name: string; note: string; colors: string[]; vars: Record<string, string> };

const palettes: Palette[] = [
  { name: 'Violet / clay', note: 'A warmer companion to the selected direction.', colors: ['#5d47b5', '#f3e4db', '#242334'], vars: { '--lab-paper': '#fbf8f5', '--lab-surface': '#fffdfa', '--lab-ink': '#2a2732', '--lab-muted': '#756d70', '--lab-line': '#e4d9d4', '--lab-accent': '#7658a9', '--lab-tint': '#f1e8f6', '--lab-bubble': '#f2e7e1', '--lab-code': '#292533', '--lab-rail': '#292531' } },
  { name: 'Cobalt / apricot', note: 'Sharper and more editorial, with a warm signal color.', colors: ['#2859a8', '#ee9b62', '#223048'], vars: { '--lab-paper': '#f7f9fc', '--lab-surface': '#ffffff', '--lab-ink': '#202c3e', '--lab-muted': '#687488', '--lab-line': '#d8e0ec', '--lab-accent': '#2d60ae', '--lab-tint': '#e5eefc', '--lab-bubble': '#e7eef8', '--lab-code': '#172335', '--lab-rail': '#1f2b3e' } },
  { name: 'Moss / parchment', note: 'Quiet, botanical, and a little more grounded.', colors: ['#47755f', '#e2c59b', '#26362f'], vars: { '--lab-paper': '#f7f7f0', '--lab-surface': '#fffef8', '--lab-ink': '#27342f', '--lab-muted': '#6d766d', '--lab-line': '#dfe2d5', '--lab-accent': '#3f7a63', '--lab-tint': '#e6f0e7', '--lab-bubble': '#e7eee2', '--lab-code': '#22302b', '--lab-rail': '#263630' } },
  { name: 'Rosewood / oat', note: 'A literary, tactile alternative with a dusky accent.', colors: ['#934d58', '#dcc79f', '#3c2a2f'], vars: { '--lab-paper': '#faf7f3', '--lab-surface': '#fffdfa', '--lab-ink': '#352a2d', '--lab-muted': '#786d6e', '--lab-line': '#e5dad5', '--lab-accent': '#984e5c', '--lab-tint': '#f4e6e3', '--lab-bubble': '#f2e5dd', '--lab-code': '#30242b', '--lab-rail': '#392a30' } },
];

function PaletteCard({ palette, dark }: { palette: Palette; dark: boolean }) {
  const vars = { ...palette.vars } as CSSProperties & Record<string, string>;
  if (dark) {
    vars['--lab-paper'] = '#1c1b21'; vars['--lab-surface'] = '#25242c'; vars['--lab-ink'] = '#f5f0ed'; vars['--lab-muted'] = '#b8afb0'; vars['--lab-line'] = '#443d43'; vars['--lab-tint'] = '#493544'; vars['--lab-bubble'] = '#40323d'; vars['--lab-code'] = '#121219'; vars['--lab-rail'] = '#17151b';
  }
  return <div className="lab-card" style={vars}>
    <div className="lab-top"><span className="lab-brand"><QuillMark /> Quill</span><span className="lab-model"><i /> gpt-4.1-mini</span></div>
    <div className="lab-thread"><div className="lab-question">How should a calm writing app feel?</div><div className="lab-reply"><div className="lab-reply-label"><span><QuillMark /></span> Quill</div><p>Like a page you can stay with: clear, warm, and steady while the answer arrives.</p><div className="lab-code">const focus = 'read → think → reply';</div></div></div>
    <div className="lab-composer">Write a message… <button><Icon name="send" /></button></div>
  </div>;
}

export function PaletteLab() {
  const [choice, setChoice] = useState('Violet / clay');
  return <div className="palette-page"><header><div className="eyebrow">Quill / palette playground</div><h1>Same calm surface, different temperature.</h1><p>These alternatives keep the reading width, hierarchy, and interaction language constant. Only the color mood changes.</p></header><main className="palette-grid">{palettes.map((palette) => <section className={'palette-option' + (choice === palette.name ? ' active' : '')} key={palette.name}><div className="palette-heading"><div><h2>{palette.name}</h2><p>{palette.note}</p></div><button onClick={() => setChoice(palette.name)}>{choice === palette.name ? 'Trying this' : 'Try this'}</button></div><div className="swatches">{palette.colors.map((color) => <span key={color} style={{ background: color }} />)}</div><div className="lab-modes"><div><span>Light</span><PaletteCard palette={palette} dark={false} /></div><div><span>Dark</span><PaletteCard palette={palette} dark /></div></div></section>)}</main></div>;
}
