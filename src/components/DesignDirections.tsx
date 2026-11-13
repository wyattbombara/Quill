import { useState } from 'react';
import { Icon, QuillMark } from './Icon';

type Direction = 'paper' | 'meadow';

type PreviewProps = {
  direction: Direction;
  dark: boolean;
};

const question = 'How should I structure a calm writing app for long answers?';
const reply = (
  <>
    <p>Give the reader one clear surface, a quiet hierarchy, and enough room for the answer to breathe. The interface should feel ready before it feels busy.</p>
    <p>I would start with three anchors:</p>
    <ol>
      <li><strong>One reading column</strong> with generous line length and paragraph rhythm.</li>
      <li><strong>A stable composer</strong> that never moves the send control while text grows.</li>
      <li><strong>A soft return path</strong> when streaming continues below the reader’s current place.</li>
    </ol>
    <div className="preview-code"><span className="code-comment">// Keep the writing surface calm</span>{'\n'}const focus = 'read → think → reply';{ '\n' }return focus;</div>
    <p>That combination makes a long answer feel like a page, not a feed.</p>
  </>
);

function Preview({ direction, dark }: PreviewProps) {
  const meadow = direction === 'meadow';
  return (
    <article className={`direction-card ${meadow ? 'meadow' : 'paper'} ${dark ? 'dark' : 'light'}`}>
      <div className="direction-window">
        <aside className="preview-rail">
          <div className="preview-brand"><span className="preview-mark"><QuillMark /></span>Quill</div>
          <button className="preview-new"><Icon name="plus" /> New chat</button>
          <div className="preview-label">Conversations</div>
          <div className="preview-chat selected"><Icon name="chat" /><span>Calm writing tools</span></div>
          <div className="preview-chat"><Icon name="chat" /><span>Notes on a new idea</span></div>
          <div className="preview-chat"><Icon name="chat" /><span>Untitled conversation with a very long name</span></div>
          <div className="preview-rail-foot"><Icon name="settings" /> Settings</div>
        </aside>
        <div className="preview-main">
          <header className="preview-toolbar"><button className="preview-icon"><Icon name="panel" /></button><strong>Calm writing tools</strong><span className="preview-model"><i /> gpt-4.1-mini <Icon name="chevron" /></span></header>
          <div className="preview-transcript">
            <div className="preview-reading">
              <div className="preview-user"><span>You</span><div>{question}</div></div>
              <div className="preview-assistant"><span className="preview-reply-mark"><QuillMark /></span><span>Quill <small>gpt-4.1-mini</small></span></div>
              <div className="preview-markdown">{reply}</div>
              <div className="preview-user follow-up"><span>You</span><div>Can you turn that into a small checklist<span className="caret" /></div></div>
            </div>
          </div>
          <div className="preview-compose"><div className="preview-draft">Can you turn that into a small checklist<span className="caret" /></div><div className="preview-actions"><button className="preview-attach"><Icon name="attach" /> Attach</button><button className="preview-send">Send <Icon name="send" /></button></div></div>
        </div>
      </div>
    </article>
  );
}

export function DesignDirections() {
  const [choice, setChoice] = useState<Direction | null>(null);
  return (
    <div className="directions-page">
      <header className="directions-header">
        <div><div className="eyebrow">Quill / design study</div><h1>Two calm ways to read.</h1><p>The same conversation, rendered in two directions. Compare the light and dark modes, then choose the one to carry into the full app.</p></div>
        <div className="direction-choice" aria-live="polite">{choice ? <><span className="choice-dot" /> {choice === 'paper' ? 'Paper, ink & violet' : 'Meadow & charcoal'} selected</> : 'Choose a direction below'}</div>
      </header>
      <main className="direction-grid">
        <section className="direction-option">
          <div className="option-heading"><div><span className="option-kicker">Direction 01</span><h2>Paper, ink & violet</h2><p>Refines Quill’s existing bookish palette with warmer paper, graphite text, and a more focused violet.</p></div><button className={choice === 'paper' ? 'chosen' : ''} onClick={() => setChoice('paper')}>{choice === 'paper' ? 'Selected' : 'Choose this'}</button></div>
          <div className="mode-label">Light mode</div><Preview direction="paper" dark={false} />
          <div className="mode-label">Dark mode</div><Preview direction="paper" dark />
        </section>
        <section className="direction-option">
          <div className="option-heading"><div><span className="option-kicker">Direction 02</span><h2>Meadow & charcoal</h2><p>A distinct, grounded palette: sage accents, smoked blue-gray surfaces, and a humanist sans-serif voice.</p></div><button className={choice === 'meadow' ? 'chosen' : ''} onClick={() => setChoice('meadow')}>{choice === 'meadow' ? 'Selected' : 'Choose this'}</button></div>
          <div className="mode-label">Light mode</div><Preview direction="meadow" dark={false} />
          <div className="mode-label">Dark mode</div><Preview direction="meadow" dark />
        </section>
      </main>
    </div>
  );
}
