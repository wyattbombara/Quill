import { useState } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import { call } from '../ipc';

const prose = [remarkGfm];
const code = [rehypeHighlight];
export default function MarkdownBody({ text }: { text: string }) {
  const [error, setError] = useState('');
  return (
    <>
      <Markdown
        remarkPlugins={prose}
        rehypePlugins={code}
        skipHtml
        components={{
          pre: ({ children }) => (
            <div className="code-frame">
              <pre tabIndex={0}>{children}</pre>
            </div>
          ),
          img: ({ alt }) => <span>{alt || 'Image'}</span>,
          a: ({ href, children }) => (
            <a
              href={href}
              onClick={(event) => {
                event.preventDefault();
                if (href)
                  void call('open_link', { url: href }).catch((e) =>
                    setError(String(e)),
                  );
              }}
            >
              {children}
            </a>
          ),
        }}
      >
        {text}
      </Markdown>
      {error && <p role="alert">{error}</p>}
    </>
  );
}
