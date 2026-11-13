export const commands = [
  { name: 'new', usage: '/new', description: 'Start a new chat' },
  { name: 'rename', usage: '/rename <title>', description: 'Rename this chat' },
  {
    name: 'model',
    usage: '/model <id>',
    description: 'Set the model for future replies',
  },
  {
    name: 'theme',
    usage: '/theme light|dark|system',
    description: 'Change appearance',
  },
  { name: 'settings', usage: '/settings', description: 'Open settings' },
  {
    name: 'goal',
    usage: '/goal [text|clear]',
    description: 'Show, set, or clear this chat’s goal',
  },
  {
    name: 'debate',
    usage: '/debate [question]',
    description: 'Choose models for Roundtable',
  },
  {
    name: 'parallel',
    usage: '/parallel [message]',
    description: 'Work with independent agents in this chat',
  },
  { name: 'status', usage: '/status', description: 'See your Quill usage' },
  { name: 'help', usage: '/help', description: 'Show commands' },
] as const;

export type CommandName = (typeof commands)[number]['name'];

export function parseInput(text: string) {
  if (text.startsWith('//'))
    return { kind: 'message', text: text.slice(1) } as const;
  const match = /^\/([a-z][\w-]*)(?:[ \t]+([^\r\n]*))?$/i.exec(text);
  if (!match || /[\r\n]/.test(text)) return { kind: 'message', text } as const;
  const command = commands.find((item) => item.name === match[1].toLowerCase());
  if (!command)
    return {
      kind: 'error',
      message:
        'Unknown command. Type /help to browse commands or add a second slash to send this as a message.',
    } as const;
  const argument = (match[2] ?? '').trim();
  if (
    command.name !== 'goal' &&
    command.name !== 'debate' &&
    command.name !== 'parallel' &&
    command.usage.includes(' ') !== Boolean(argument)
  )
    return { kind: 'error', message: 'Usage: ' + command.usage } as const;
  return { kind: 'command', name: command.name, argument } as const;
}
