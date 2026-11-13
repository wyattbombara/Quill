export function shortModel(id: string): string {
  // Preserve custom model IDs; simplify OpenAI GPT display names only.
  if (!/^gpt[- ]/i.test(id)) return id;
  return id
    .replace(/^gpt[- ]/i, '')
    .replace(/-\d{4}-\d{2}-\d{2}$/, '')
    .replace(/-/g, ' ');
}

export const modelGroups = {
  'GPT-6': [
    'gpt-6-astra',
    'gpt-6.1-sol',
    'gpt-6.1-terra',
    'gpt-6-luna',
    'gpt-6-sol',
  ],
  'GPT-5': [
    'gpt-5.6-sol',
    'gpt-5.6-terra',
    'gpt-5.6-luna',
    'gpt-5.5',
    'gpt-5.4',
    'gpt-5.4-mini',
  ],
  'GPT-4': ['gpt-4.1-mini', 'gpt-4.1', 'gpt-4o'],
};
