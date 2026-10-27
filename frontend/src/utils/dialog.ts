import { open, save } from '@tauri-apps/plugin-dialog';

export interface FileFilter {
  name: string;
  extensions: string[];
}

export async function pickFile(filters?: FileFilter[], multiple = false): Promise<string[] | null> {
  const result = await open({ multiple, directory: false, filters });
  if (result === null) return null;
  if (Array.isArray(result)) return result;
  return [result];
}

export async function pickDirectory(): Promise<string | null> {
  const result = await open({ multiple: false, directory: true });
  return typeof result === 'string' ? result : null;
}

export async function saveFile(defaultPath?: string, filters?: FileFilter[]): Promise<string | null> {
  const result = await save({ defaultPath, filters });
  return result ?? null;
}

export function envPackageFilter(): FileFilter[] {
  return [{ name: 'FinEnv 加密环境包', extensions: ['finenv'] }];
}

export function backupFilter(): FileFilter[] {
  return [{ name: 'FinEnv 备份', extensions: ['finenvbackup'] }];
}

export function jsonFilter(): FileFilter[] {
  return [{ name: 'JSON', extensions: ['json'] }];
}

export function csvFilter(): FileFilter[] {
  return [{ name: 'CSV', extensions: ['csv'] }];
}

export function accountPackageFilter(): FileFilter[] {
  return [{ name: 'FinEnv 加密账号包', extensions: ['finenv'] }];
}
