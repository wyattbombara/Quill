import type { ReactNode } from 'react';

const paths = {
  copy: <path d="M9 8h11v13H9zM15 8V3H4v13h5" />,
  search: (
    <>
      <circle cx="10" cy="10" r="6" />
      <path d="m15 15 6 6" />
    </>
  ),
  usage: <path d="M3 20h18M6 16v-5m6 5V4m6 12V8" />,
  keyboard: <path d="M2 6h20v12H2z M5 10h1m2 0h1m2 0h1m2 0h1m2 0h1M7 14h10" />,
  goal: <path d="M5 20V4m0 1c5-4 9 4 15 0l-2 7c-5 4-9-4-13 0" />,
  panel: (
    <>
      <rect x="3" y="4" width="18" height="16" rx="3" />
      <path d="M9 4v16" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  chat: (
    <path d="M6 4.5h12a3 3 0 0 1 3 3v7a3 3 0 0 1-3 3H9l-5.5 3v-4a3 3 0 0 1-1-2v-7a3 3 0 0 1 3.5-3Z" />
  ),
  settings: (
    <>
      <path d="M4 6h8m5 0h3M4 12h3m5 0h8M4 18h9m5 0h2" />
      <circle cx="14.5" cy="6" r="2.3" />
      <circle cx="9.5" cy="12" r="2.3" />
      <circle cx="15.5" cy="18" r="2.3" />
    </>
  ),
  attach: (
    <path d="m8 13 6.4-6.4a3 3 0 0 1 4.2 4.2l-8.2 8.3a5 5 0 0 1-7-7l8.4-8.5a2 2 0 0 1 .5-.4" />
  ),
  send: <path d="M12 19V5m-6 6 6-6 6 6" />,
  down: <path d="M12 5v14m-6-6 6 6 6-6" />,
  chevron: <path d="m7 10 5 5 5-5" />,
  stop: (
    <rect
      x="6"
      y="6"
      width="12"
      height="12"
      rx="2"
      fill="currentColor"
      stroke="none"
    />
  ),
  write: (
    <>
      <path d="m4 16-.5 4.5 4.5-.8L19 8l-4-4Z" />
      <path d="m12.6 6.5 4 4" />
    </>
  ),
  explain: (
    <>
      <path d="M12 6c-2-2-6-2.5-9-1.5V18c3-1 6.5-.5 9 1.5 2.5-2 6-2.5 9-1.5V4.5C17.5 3.5 14 4 12 6Z" />
      <path d="M12 6v13" />
    </>
  ),
  ideas: (
    <>
      <path d="M9 16c0-2-3-3-3-7a6 6 0 0 1 12 0c0 4-3 5-3 7ZM10 20h4" />
    </>
  ),
  close: <path d="m6 6 12 12M18 6 6 18" />,
  moon: <path d="M20 15.5A8.5 8.5 0 1 1 8.5 4 6.7 6.7 0 0 0 20 15.5Z" />,
  sun: (
    <>
      <circle cx="12" cy="12" r="3.5" />
      <path d="M12 2.5v2M12 19.5v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2.5 12h2M19.5 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
    </>
  ),
  trash: (
    <>
      <path d="M4 6h16M9 6V3.5h6V6M6 6l1 14h10l1-14M10 10v6m4-6v6" />
    </>
  ),
} satisfies Record<string, ReactNode>;

export function Icon({ name }: { name: keyof typeof paths }) {
  return (
    <svg
      className="icon"
      aria-hidden="true"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.65"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {paths[name]}
    </svg>
  );
}

export function QuillMark() {
  return (
    <svg
      className="quill-symbol"
      viewBox="0 0 64 64"
      fill="none"
      aria-hidden="true"
    >
      <path
        fill="currentColor"
        d="M11 47C16 31 24 16 41 9c6-2 12-3 17-3-1 7-4 15-9 22l-11 2 7 2c-6 8-14 13-26 15l-8 9-3-2 31-34C25 29 18 39 11 47Z"
      />
    </svg>
  );
}
