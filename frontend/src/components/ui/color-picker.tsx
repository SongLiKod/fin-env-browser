import { Check, Palette } from 'lucide-react';
import { cn } from '@/utils';
import { ENV_COLOR_PALETTE, isValidHexColor } from '@/utils/environment';

export function ColorPicker({
  value,
  onChange,
  allowAuto = true,
}: {
  value: string;
  onChange: (value: string) => void;
  allowAuto?: boolean;
}) {
  const normalized = value.trim();
  const valid = isValidHexColor(normalized);
  const swatch = valid ? normalized.toUpperCase() : '#2563EB';

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {allowAuto ? (
        <button
          type="button"
          onClick={() => onChange('')}
          className={cn(
            'h-7 rounded-md border px-2 text-[10px] transition-colors',
            normalized
              ? 'border-border text-content-weak hover:border-primary/50'
              : 'border-primary bg-primary-light text-primary',
          )}
        >
          自动
        </button>
      ) : null}
      {ENV_COLOR_PALETTE.map((color) => {
        const selected = normalized.toUpperCase() === color;
        return (
          <button
            key={color}
            type="button"
            aria-label={`选择颜色 ${color}`}
            title={color}
            onClick={() => onChange(color)}
            className={cn(
              'flex h-6 w-6 items-center justify-center rounded-full border-2 transition-transform hover:scale-110',
              selected ? 'border-content-main' : 'border-transparent',
            )}
            style={{ backgroundColor: color }}
          >
            {selected ? <Check className="h-3 w-3 text-white drop-shadow" /> : null}
          </button>
        );
      })}
      <label
        className="relative flex h-7 w-7 cursor-pointer items-center justify-center rounded-md border border-border text-content-weak hover:border-primary/60 hover:text-primary"
        title="自定义颜色"
      >
        <input
          type="color"
          className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
          value={swatch}
          onChange={(event) => onChange(event.target.value.toUpperCase())}
        />
        <Palette className="h-3.5 w-3.5" />
      </label>
      <input
        value={normalized}
        onChange={(event) => onChange(event.target.value)}
        placeholder="#2563EB"
        maxLength={7}
        className="h-7 w-[92px] rounded-md border border-border bg-surface-base px-2 font-mono text-[11px] uppercase text-content-main outline-none focus:border-primary"
      />
    </div>
  );
}
