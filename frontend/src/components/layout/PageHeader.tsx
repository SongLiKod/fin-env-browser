import * as React from 'react';
import { ChevronLeft } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';

export function PageHeader({
  title,
  description,
  actions,
  back,
  eyebrow,
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  actions?: React.ReactNode;
  back?: boolean;
  eyebrow?: React.ReactNode;
}) {
  const navigate = useNavigate();
  return (
    <div className="flex flex-wrap items-start justify-between gap-3">
      <div className="flex min-w-0 items-start gap-2">
        {back ? (
          <Button variant="ghost" size="icon-sm" className="mt-0.5" onClick={() => navigate(-1)}>
            <ChevronLeft className="h-4 w-4" />
          </Button>
        ) : null}
        <div className="min-w-0">
          {eyebrow ? (
            <div className="page-eyebrow mb-1 text-[10px] font-semibold uppercase tracking-widest text-content-hint">
              {eyebrow}
            </div>
          ) : null}
          <h2 className="page-title truncate text-xl font-semibold tracking-tight text-content-main">{title}</h2>
          {description ? (
            <p className="mt-1 max-w-2xl text-xs leading-5 text-content-weak">{description}</p>
          ) : null}
        </div>
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  );
}
