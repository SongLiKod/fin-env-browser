import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';
import type {
  AccountStatus,
  EnvStatus,
  PermissionKey,
  Role,
} from '@/ipc/types';

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

export function formatDate(value: string | null | undefined): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(date);
}

export function formatDateShort(value: string | null | undefined): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(date);
}

export function relativeTime(value: string | null | undefined): string {
  if (!value) return '从未';
  const timestamp = new Date(value).getTime();
  if (Number.isNaN(timestamp)) return value;
  const diff = Date.now() - timestamp;
  if (diff < 0) return formatDateShort(value);
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds} 秒前`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return formatDateShort(value);
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value >= 10 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return '0s';
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  if (minutes < 60) return `${minutes}m ${rest}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

export function parseTags(value: string): string[] {
  return Array.from(
    new Set(
      value
        .split(/[,，\s]+/)
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  );
}

export function truncate(value: string, length = 48): string {
  if (value.length <= length) return value;
  return `${value.slice(0, length)}…`;
}

export function parseCsvRows(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';
  let quoted = false;
  for (let i = 0; i < text.length; i += 1) {
    const char = text[i];
    if (quoted) {
      if (char === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 1;
        } else {
          quoted = false;
        }
      } else {
        field += char;
      }
    } else if (char === '"') {
      quoted = true;
    } else if (char === ',') {
      row.push(field);
      field = '';
    } else if (char === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
    } else if (char !== '\r') {
      field += char;
    }
  }
  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows.filter((item) => item.some((cell) => cell.trim() !== ''));
}

export async function copyText(value: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(value);
    return true;
  } catch {
    try {
      const area = document.createElement('textarea');
      area.value = value;
      area.style.position = 'fixed';
      area.style.opacity = '0';
      document.body.appendChild(area);
      area.select();
      document.execCommand('copy');
      document.body.removeChild(area);
      return true;
    } catch {
      return false;
    }
  }
}

export const roleLabels: Record<Role, string> = {
  super_admin: '超级管理员',
  test_lead: '测试负责人',
  test_engineer: '测试工程师',
  viewer: '只读查看者',
};

export const permissionLabels: Record<PermissionKey, string> = {
  view_list: '查看列表',
  open_close: '打开/关闭环境',
  create_env: '创建环境',
  edit_env: '编辑环境',
  delete_env: '删除环境',
  clone_env: '克隆环境',
  edit_fingerprint_proxy: '修改指纹与代理',
  export_sensitive: '导出 Cookie/账号',
  audit_view: '查看审计日志',
  settings_manage: '系统设置',
  user_manage: '用户与角色',
  api_key_manage: 'API 密钥管理',
};

export const envStatusMeta: Record<EnvStatus, { label: string; tone: 'success' | 'muted' | 'warning' }> = {
  active: { label: '已启用', tone: 'success' },
  disabled: { label: '已停用', tone: 'muted' },
  archived: { label: '已归档', tone: 'warning' },
};

export const accountStatusMeta: Record<
  AccountStatus,
  { label: string; tone: 'success' | 'primary' | 'warning' | 'muted' | 'danger' }
> = {
  pending: { label: '待启用', tone: 'muted' },
  in_use: { label: '使用中', tone: 'success' },
  frozen: { label: '冻结', tone: 'warning' },
  deprecated: { label: '废弃', tone: 'danger' },
};

export const testEnvLabels: Record<string, string> = {
  dev: '开发 dev',
  test: '测试 test',
  pre: '预发 pre',
  prod: '生产 prod',
};

export const auditActionLabels: Record<string, string> = {
  'env.create': '创建环境',
  'env.update': '更新环境',
  'env.delete': '删除环境',
  'env.clone': '克隆环境',
  'env.import': '导入环境',
  'env.export': '导出环境',
  'env.ip_check': '检测出口 IP',
  'env.assign_group': '调整分组',
  'browser.open': '打开浏览器',
  'browser.close': '关闭浏览器',
  'browser.crash': '进程崩溃',
  'browser.recycle': '强制回收',
  'browser.idle_close': '闲置回收',
  'browser.memory_close': '内存超限关闭',
  'browser.screenshot': '截图',
  'browser.recording': '录屏',
  'browser.pre_script': '前置脚本',
  'browser.reconnect': '接管浏览器',
  'cookie.read': '读取 Cookie',
  'cookie.write': '写入 Cookie',
  'cookie.export': '导出 Cookie',
  'account.create': '新增账号',
  'account.update': '更新账号',
  'account.delete': '删除账号',
  'account.reveal': '查看密码',
  'account.import': '导入账号',
  'account.export': '导出账号',
  'proxy.create': '新增代理',
  'proxy.update': '更新代理',
  'proxy.delete': '删除代理',
  'proxy.check': '检测代理',
  'proxy.assign': '分配代理',
  'proxy.align': '代理对齐',
  'fingerprint.update': '更新指纹',
  'fingerprint.detect': '指纹检测',
  'snapshot.create': '创建快照',
  'snapshot.restore': '还原快照',
  'snapshot.delete': '删除快照',
  'hosts.update': '更新 Hosts',
  'mock.update': '更新 Mock',
  'extension.add': '安装扩展',
  'extension.assign': '分配扩展',
  'extension.unassign': '取消扩展',
  'extension.delete': '删除扩展',
  'captcha.handle': '处理验证码',
  'settings.update': '更新设置',
  'theme.update': '更新主题',
  'user.create': '创建用户',
  'user.update': '更新用户',
  'user.delete': '删除用户',
  'auth.login': '登录',
  'auth.login_failed': '登录失败',
  'auth.logout': '退出登录',
  'auth.password_change': '修改密码',
  'auth.bootstrap': '初始化管理员',
  'api_key.create': '创建 API 密钥',
  'api_key.delete': '吊销 API 密钥',
  'api.start': '启动 API',
  'api.stop': '停止 API',
  'api.request': 'API 请求',
  'backup.create': '创建备份',
  'backup.restore': '恢复备份',
  'backup.upgrade': '升级前备份',
  'sync.push': '同步上传',
  'sync.pull': '同步下载',
  'har.export': '导出 HAR',
  'artifact.open_dir': '打开产物目录',
  'audit.export': '导出审计',
  'group.create': '创建分组',
  'group.update': '更新分组',
  'group.delete': '删除分组',
};
