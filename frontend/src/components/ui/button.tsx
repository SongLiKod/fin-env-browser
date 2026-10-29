import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';
import { Loader2 } from 'lucide-react';
import { cn } from '@/utils';

const buttonVariants = cva(
  'inline-flex select-none items-center justify-center gap-1.5 whitespace-nowrap rounded-md border text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-light disabled:pointer-events-none disabled:opacity-50 active:translate-y-[0.5px]',
  {
    variants: {
      variant: {
        primary:
          'primary-button border-transparent bg-primary text-[var(--text-on-primary)] shadow-sm hover:bg-primary-dark',
        secondary:
          'border-border bg-surface-base text-content-main shadow-sm hover:bg-surface-secondary',
        outline:
          'border-border-strong bg-transparent text-content-main hover:bg-surface-secondary',
        ghost: 'border-transparent bg-transparent text-content-weak hover:bg-surface-tertiary hover:text-content-main',
        danger: 'border-transparent bg-danger text-[var(--text-on-danger)] hover:brightness-95',
        soft: 'border-transparent bg-primary-light text-primary hover:brightness-[0.98]',
      },
      size: {
        sm: 'h-7 px-2.5 text-xs',
        default: 'h-[var(--control-height)] px-3.5',
        lg: 'h-10 px-5',
        icon: 'h-[var(--control-height)] w-[var(--control-height)] p-0',
        'icon-sm': 'h-7 w-7 p-0',
      },
    },
    defaultVariants: {
      variant: 'secondary',
      size: 'default',
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  loading?: boolean;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, loading, children, disabled, type = 'button', ...props }, ref) => (
    <button
      ref={ref}
      type={type}
      className={cn(buttonVariants({ variant, size }), className)}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
      {children}
    </button>
  ),
);
Button.displayName = 'Button';

export { buttonVariants };
