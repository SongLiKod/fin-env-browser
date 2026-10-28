import * as React from 'react';
import { LockKeyhole, RefreshCw, ShieldCheck, Trash2, UserPlus, Users } from 'lucide-react';
import { ipc } from '@/ipc/client';
import type {
  EnvironmentMember,
  EnvironmentMemberCandidate,
  EnvironmentView,
} from '@/ipc/types';
import { useAuthStore } from '@/store/auth';
import { toastError, useToastStore } from '@/store/toast';
import { roleLabels } from '@/utils';
import { envHasAccess } from '@/utils/environment';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardBody, CardHeader, EmptyState, Spinner } from '@/components/ui/card';
import { Field, Select } from '@/components/ui/input';

const accessMeta: Record<string, { label: string; tone: 'neutral' | 'primary' | 'success'; hint: string }> = {
  view: { label: '查看', tone: 'neutral', hint: '可查看环境详情与账号列表，不能启动或修改' },
  operate: { label: '操作', tone: 'primary', hint: '可启动/关闭、截图、导航、维护账号与 Hosts/Mock' },
  manage: { label: '管理', tone: 'success', hint: '可修改环境、指纹、代理、快照、扩展与成员' },
};

export function AccessPanel({ env }: { env: EnvironmentView }) {
  const can = useAuthStore((state) => state.can);
  const currentUser = useAuthStore((state) => state.session?.user);
  const toast = useToastStore();
  const [members, setMembers] = React.useState<EnvironmentMember[]>([]);
  const [candidates, setCandidates] = React.useState<EnvironmentMemberCandidate[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [manage, setManage] = React.useState(false);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [newUser, setNewUser] = React.useState('');
  const [newLevel, setNewLevel] = React.useState<'view' | 'operate' | 'manage'>('operate');

  const load = React.useCallback(async () => {
    setLoading(true);
    try {
      const list = await ipc.envMemberList(env.id);
      setMembers(list);
      if (can('manage_env_members') && envHasAccess(env, 'manage')) {
        try {
          setCandidates(await ipc.envMemberCandidates(env.id));
          setManage(true);
        } catch {
          setCandidates([]);
          setManage(false);
        }
      } else {
        setCandidates([]);
        setManage(false);
      }
    } catch (error) {
      toastError(error);
    } finally {
      setLoading(false);
    }
  }, [env.id, can]);

  React.useEffect(() => {
    void load();
  }, [load]);

  React.useEffect(() => {
    if (newUser && members.some((member) => member.userId === newUser)) {
      setNewUser('');
    }
  }, [members, newUser]);

  const addable = candidates.filter(
    (candidate) => !members.some((member) => member.userId === candidate.userId),
  );

  const setLevel = async (userId: string, accessLevel: 'view' | 'operate' | 'manage') => {
    setBusy(userId);
    try {
      const next = await ipc.envMemberSet(env.id, { userId, accessLevel });
      setMembers(next);
      toast.success('环境权限已更新');
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(null);
    }
  };

  const remove = async (member: EnvironmentMember) => {
    setBusy(member.userId);
    try {
      const next = await ipc.envMemberRemove(env.id, member.userId);
      setMembers(next);
      toast.success('已移除环境成员', member.displayName);
    } catch (error) {
      toastError(error);
    } finally {
      setBusy(null);
    }
  };

  const add = async () => {
    if (!newUser) return;
    await setLevel(newUser, newLevel);
    setNewUser('');
  };

  return (
    <div className="space-y-3">
      <Card>
        <CardHeader
          title={
            <span className="flex items-center gap-1.5">
              <Users className="h-4 w-4 text-primary" /> 访问成员
            </span>
          }
          description="非超级管理员只能访问被分配的环境；访问级别在角色权限之上进一步限制操作范围。"
          actions={
            <Button variant="ghost" size="sm" onClick={() => void load()} disabled={loading}>
              <RefreshCw className={loading ? 'h-3.5 w-3.5 animate-spin' : 'h-3.5 w-3.5'} /> 刷新
            </Button>
          }
        />
        <CardBody className="space-y-3">
          {loading && members.length === 0 ? (
            <div className="flex items-center justify-center gap-2 py-10 text-xs text-content-hint">
              <Spinner /> 正在加载成员…
            </div>
          ) : members.length === 0 ? (
            <EmptyState
              icon={<LockKeyhole className="h-5 w-5" />}
              title="还没有分配团队成员"
              description="除超级管理员外，未分配的成员无法在环境列表中看到此环境。"
            />
          ) : (
            <div className="divide-y divide-border rounded-lg border border-border">
              {members.map((member) => {
                const meta = accessMeta[member.accessLevel] ?? accessMeta.view;
                const isSelf = member.userId === currentUser?.id;
                return (
                  <div key={member.userId} className="flex flex-wrap items-center gap-3 px-3 py-2.5">
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary-light text-xs font-semibold text-primary">
                      {member.displayName.slice(0, 1).toUpperCase()}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <span className="text-xs font-medium text-content-main">{member.displayName}</span>
                        <span className="text-[10px] text-content-hint">@{member.username}</span>
                        <Badge tone="outline">{roleLabels[member.role] ?? member.role}</Badge>
                        {!member.enabled ? <Badge tone="warning">已禁用</Badge> : null}
                        {isSelf ? <Badge tone="neutral">当前用户</Badge> : null}
                      </div>
                      <p className="mt-0.5 text-[10px] text-content-hint">{meta.hint}</p>
                    </div>
                    {manage && !isSelf ? (
                      <Select
                        wrapperClassName="w-[104px]"
                        className="h-7 text-xs"
                        value={member.accessLevel}
                        onChange={(event) =>
                          void setLevel(
                            member.userId,
                            event.target.value as 'view' | 'operate' | 'manage',
                          )
                        }
                        disabled={busy === member.userId}
                      >
                        <option value="view">查看</option>
                        <option value="operate">操作</option>
                        <option value="manage">管理</option>
                      </Select>
                    ) : (
                      <Badge tone={meta.tone}>{meta.label}</Badge>
                    )}
                    {manage && !isSelf ? (
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        className="text-danger"
                        title="移除环境访问权限"
                        loading={busy === member.userId}
                        onClick={() => void remove(member)}
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    ) : null}
                  </div>
                );
              })}
            </div>
          )}

          {manage ? (
            <div className="flex flex-wrap items-end gap-2 rounded-lg border border-dashed border-border p-3">
              <Field label="添加团队成员" className="min-w-[190px] flex-1">
                <Select
                  value={newUser}
                  onChange={(event) => setNewUser(event.target.value)}
                  placeholder={addable.length ? '选择团队成员' : '所有成员均已分配'}
                >
                  {addable.map((candidate) => (
                    <option key={candidate.userId} value={candidate.userId}>
                      {candidate.displayName} · @{candidate.username} · {roleLabels[candidate.role] ?? candidate.role}
                      {candidate.enabled ? '' : '（已禁用）'}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label="访问级别" className="w-[120px]">
                <Select
                  value={newLevel}
                  onChange={(event) =>
                    setNewLevel(event.target.value as 'view' | 'operate' | 'manage')
                  }
                >
                  <option value="view">查看</option>
                  <option value="operate">操作</option>
                  <option value="manage">管理</option>
                </Select>
              </Field>
              <Button
                variant="primary"
                disabled={!newUser}
                onClick={() => void add()}
              >
                <UserPlus className="h-4 w-4" /> 添加
              </Button>
            </div>
          ) : null}
        </CardBody>
      </Card>

      <div className="flex items-start gap-2 rounded-lg border border-border bg-surface-secondary/60 p-3 text-[11px] leading-5 text-content-hint">
        <ShieldCheck className="mt-0.5 h-3.5 w-3.5 shrink-0 text-primary" />
        <p>
          超级管理员始终可以访问全部环境；测试负责人、测试工程师和只读查看者需要被分配到环境。
          最终操作仍受全局角色权限限制，例如只读查看者即使获得「操作」级别也不能启动浏览器。
        </p>
      </div>
    </div>
  );
}
