import { forwardRef } from 'react';

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label: string;
  error?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ label, error, className = '', ...props }, ref) => {
    return (
      <div className="flex flex-col gap-1">
        <label className="text-xs text-white/60">{label}</label>
        <input
          ref={ref}
          className={`px-3 py-2 bg-white/5 border border-white/10 rounded text-sm text-white placeholder-white/30 focus:outline-none focus:border-white/30 transition-colors ${error ? 'border-red-500/50' : ''} ${className}`}
          {...props}
        />
        {error && <span className="text-xs text-red-400">{error}</span>}
      </div>
    );
  }
);

Input.displayName = 'Input';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary';
  loading?: boolean;
}

export function Button({
  variant = 'primary',
  loading,
  className = '',
  children,
  disabled,
  ...props
}: ButtonProps) {
  const baseStyles = 'px-4 py-2 text-sm font-medium rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed';
  const variantStyles = {
    primary: 'bg-white text-black hover:bg-white/90',
    secondary: 'bg-white/10 text-white hover:bg-white/20 border border-white/10',
  };

  return (
    <button
      className={`${baseStyles} ${variantStyles[variant]} ${className}`}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? 'Loading...' : children}
    </button>
  );
}

interface FormProps extends React.FormHTMLAttributes<HTMLFormElement> {
  title: string;
  subtitle?: string;
  footer?: React.ReactNode;
}

export function Form({ title, subtitle, footer, children, className = '', ...props }: FormProps) {
  return (
    <div className="w-full max-w-sm mx-auto">
      <div className="bg-zinc-900/50 border border-white/10 rounded-lg p-6">
        <div className="mb-6">
          <h1 className="text-xl font-medium text-white">{title}</h1>
          {subtitle && <p className="mt-1 text-sm text-white/60">{subtitle}</p>}
        </div>
        <form className={`flex flex-col gap-4 ${className}`} {...props}>
          {children}
        </form>
      </div>
      {footer && <div className="mt-4 text-center text-sm text-white/60">{footer}</div>}
    </div>
  );
}
