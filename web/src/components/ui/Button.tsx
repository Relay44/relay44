import { ButtonHTMLAttributes, forwardRef } from 'react';
import { cn } from '@/lib/utils';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'outline' | 'success' | 'danger' | 'bid' | 'ask';
  size?: 'sm' | 'md' | 'lg' | 'xl';
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      className,
      variant = 'primary',
      size = 'md',
      loading,
      disabled,
      children,
      ...props
    },
    ref
  ) => {
    const baseStyles = cn(
      'inline-flex items-center justify-center font-medium',
      'transition-all duration-fast ease-out',
      'disabled:opacity-50 disabled:cursor-not-allowed disabled:pointer-events-none',
      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-bg-base'
    );

    const variants = {
      primary: cn(
        'bg-accent text-white',
        'hover:bg-accent-hover'
      ),
      secondary: cn(
        'bg-bg-secondary text-text-primary',
        'border border-border hover:border-border-hover hover:bg-bg-tertiary'
      ),
      ghost: cn(
        'text-text-secondary',
        'hover:text-text-primary hover:bg-bg-secondary'
      ),
      outline: cn(
        'bg-transparent text-accent',
        'border border-accent hover:bg-accent-muted'
      ),
      success: cn(
        'bg-accent text-white',
        'hover:bg-accent-hover'
      ),
      danger: cn(
        'bg-bg-tertiary text-text-primary',
        'border border-border hover:bg-bg-secondary'
      ),
      bid: cn(
        'bg-bid text-white font-semibold',
        'hover:bg-bid-hover'
      ),
      ask: cn(
        'bg-ask text-white font-semibold',
        'hover:bg-ask-hover'
      ),
    };

    const sizes = {
      sm: 'h-8 px-3 text-sm gap-1.5',
      md: 'h-10 px-4 text-base gap-2',
      lg: 'h-12 px-6 text-lg gap-2',
      xl: 'h-14 px-8 text-lg gap-2.5',
    };

    return (
      <button
        ref={ref}
        className={cn(baseStyles, variants[variant], sizes[size], className)}
        disabled={disabled || loading}
        {...props}
      >
        {loading ? (
          <Spinner size={size === 'sm' ? 'sm' : 'md'} />
        ) : null}
        {children}
      </button>
    );
