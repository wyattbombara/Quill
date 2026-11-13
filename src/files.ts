import type { Attachment } from './ipc';

const types: Record<string, string> = {
  pdf: 'application/pdf',
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  webp: 'image/webp',
  txt: 'text/plain',
  md: 'text/markdown',
  csv: 'text/csv',
  json: 'application/json',
};
export const accepts = Object.keys(types)
  .map((ext) => '.' + ext)
  .join(',');
export function validateFiles(files: File[]) {
  if (files.length > 6) throw new Error('Choose up to six attachments.');
  if (files.reduce((total, file) => total + file.size, 0) > 16 * 1024 ** 2)
    throw new Error('Attachments exceed 16 MiB total.');
  for (const file of files) {
    if (!types[file.name.split('.').pop()?.toLowerCase() ?? ''])
      throw new Error('Supported files: ' + accepts);
    if (!file.size || file.size > 8 * 1024 ** 2)
      throw new Error(file.name + ' must be nonempty and at most 8 MiB.');
  }
}
export function encode(file: File): Promise<Attachment> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error('Could not read ' + file.name));
    reader.onload = () =>
      resolve({
        name: file.name,
        mime: types[file.name.split('.').pop()!.toLowerCase()],
        data: String(reader.result).split(',')[1],
      });
    reader.readAsDataURL(file);
  });
}
