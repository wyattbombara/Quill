import { Icon, QuillMark } from './Icon';

export function KeyboardGuide({
  modifierSend = false,
  shortcut,
  available,
}: {
  modifierSend?: boolean;
  shortcut: string;
  available: boolean | null;
}) {
  return (
    <section className="keyboard-guide">
      <div className="guide-by">
        <span className="reply-mark">
          <QuillMark />
        </span>
        Quill
      </div>
      <h1>Quill, from the keyboard.</h1>
      <p>
        {available === false
          ? 'Choose an available global shortcut in Settings. You can use these commands in the composer.'
          : 'Open the app from anywhere, then keep writing without reaching for the mouse.'}
      </p>
      <div className="shortcut">
        <div className="shortcut-inner">
          <div>
            <small>
              <Icon name="keyboard" />
              Global shortcut
            </small>
            <strong>
              {available === false
                ? 'Shortcut unavailable'
                : 'Bring Quill into focus'}
            </strong>
          </div>
          <div className="keyset" aria-label={shortcut}>
            {shortcut.split('+').map((key, i) => (
              <kbd key={i}>{key.trim()}</kbd>
            ))}
          </div>
        </div>
      </div>
      <dl className="keyrows">
        <div><dt>{modifierSend && <><kbd>{navigator.platform.includes('Mac') ? '⌘' : 'Ctrl'}</kbd> + </>}<kbd>Enter</kbd></dt><dd>Send your message.</dd></div>
        <div>
          <dt>
            <kbd>/</kbd>
          </dt>
          <dd>Browse commands from the composer.</dd>
        </div>
        <div>
          <dt>
            <kbd>↑</kbd> <kbd>↓</kbd> <kbd>Enter</kbd>
          </dt>
          <dd>Choose a command and run it.</dd>
        </div>
        <div>
          <dt>
            <kbd>Shift</kbd> + <kbd>Enter</kbd>
          </dt>
          <dd>Add a new line to your message.</dd>
        </div>
      </dl>
    </section>
  );
}
