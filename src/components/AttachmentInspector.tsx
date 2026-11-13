import { useEffect, useRef, useState } from 'react';
import { call, type Attachment, type Turn } from '../ipc';
import { Icon } from './Icon';

export function AttachmentInspector({
  chat,
  turns,
  close,
}: {
  chat: number | null;
  turns: Turn[];
  close: () => void;
}) {
  const files = turns.flatMap((turn) =>
    turn.files.map((name, position) => ({
      name,
      turn: turn.id,
      position,
      key: turn.id + '/' + position,
    })),
  );
  const [choice, choose] = useState('');
  const selected = files.find((f) => f.key === choice) ?? files.at(-1);
  const turn = selected?.turn;
  const position = selected?.position;
  const [loaded, setLoaded] = useState<{
    key: string;
    file?: Attachment;
    error?: string;
  }>();
  const [dimensions, setDimensions] = useState('');
  const zoom = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    let current = true;
    setDimensions('');
    zoom.current?.close();
    if (chat !== null && turn !== undefined && position !== undefined) {
      const key = turn + '/' + position;
      call('read_attachment', { chat, turn, position })
        .then((file) => {
          if (current) setLoaded({ key, file });
        })
        .catch((e) => {
          if (current) setLoaded({ key, error: String(e) });
        });
    }
    return () => {
      current = false;
    };
  }, [chat, turn, position]);
  const result = loaded?.key === selected?.key ? loaded : undefined;
  const file = result?.file;
  const image =
    file && ['image/png', 'image/jpeg', 'image/webp'].includes(file.mime)
      ? 'data:' + file.mime + ';base64,' + file.data
      : null;
  const bytes = file
    ? (file.data.length * 3) / 4 - (file.data.match(/=*$/)?.[0].length ?? 0)
    : 0;
  return (
    <section className="file-inspector" aria-label="Conversation attachments">
      <header>
        <h2>Attachments</h2>
        <span>{files.length}</span>
        <button
          title="Close attachments"
          aria-label="Close attachments"
          onClick={close}
        >
          ×
        </button>
      </header>
      {!selected ? (
        <p className="status">No files in this conversation.</p>
      ) : (
        <>
          {files.length > 1 && (
            <select
              value={selected.key}
              aria-label="Select attachment"
              onChange={(e) => choose(e.target.value)}
            >
              {files.map((f) => (
                <option value={f.key} key={f.key}>
                  {f.name}
                </option>
              ))}
            </select>
          )}
          {image && (
            <button
              className="file-preview"
              title="Expand image"
              onClick={() => zoom.current?.showModal()}
            >
              <img
                key={selected.key}
                src={image}
                alt={selected.name}
                onLoad={(e) =>
                  setDimensions(
                    e.currentTarget.naturalWidth +
                      ' × ' +
                      e.currentTarget.naturalHeight,
                  )
                }
              />
            </button>
          )}
          <strong className="file-name">
            <Icon name="attach" />
            {selected.name}
          </strong>
          {file && (
            <dl>
              <dt>Format</dt>
              <dd>{file.mime.split('/')[1].toUpperCase()}</dd>
              {image && dimensions && (
                <>
                  <dt>Dimensions</dt>
                  <dd>{dimensions}</dd>
                </>
              )}
              <dt>Size</dt>
              <dd>
                {bytes < 1024
                  ? bytes + ' B'
                  : (bytes / 1024).toLocaleString(undefined, {
                      maximumFractionDigits: 1,
                    }) + ' KB'}
              </dd>
            </dl>
          )}
          {!file && (
            <p role="status">{result?.error ?? 'Loading attachment…'}</p>
          )}
          {image && (
            <dialog
              ref={zoom}
              className="image-dialog"
              aria-label={selected.name}
            >
              <header>
                <strong>{selected.name}</strong>
                <button autoFocus onClick={() => zoom.current?.close()}>
                  Close
                </button>
              </header>
              <img src={image} alt={selected.name} />
            </dialog>
          )}
        </>
      )}
    </section>
  );
}
