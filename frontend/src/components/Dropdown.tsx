import { useState, useRef, useEffect, type ReactNode } from 'react';

interface DropdownProps {
  trigger: ReactNode;
  children: ReactNode;
  align?: 'left' | 'right';
}

export default function Dropdown({ trigger, children, align = 'right' }: DropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleMouseEnter = () => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    setIsOpen(true);
  };

  const handleMouseLeave = () => {
    timeoutRef.current = setTimeout(() => {
      setIsOpen(false);
    }, 150);
  };

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  return (
    <div
      ref={containerRef}
      className="relative"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      <div className="cursor-pointer">{trigger}</div>
      {isOpen && (
        <div
          className={`absolute top-full mt-1 min-w-[160px] bg-zinc-900 border border-white/10 rounded-lg shadow-xl overflow-hidden z-50 ${
            align === 'right' ? 'right-0' : 'left-0'
          }`}
        >
          {children}
        </div>
      )}
    </div>
  );
}

interface DropdownItemProps {
  children: ReactNode;
  onClick?: () => void;
}

export function DropdownItem({ children, onClick }: DropdownItemProps) {
  return (
    <button
      onClick={onClick}
      className="w-full px-3 py-1.5 text-left text-xs text-white/60 hover:text-white hover:bg-white/5 transition-colors flex items-center gap-2"
    >
      {children}
    </button>
  );
}

export function DropdownLabel({ children }: { children: ReactNode }) {
  return (
    <div className="px-3 py-2 text-xs text-white/40 border-b border-white/10">
      {children}
    </div>
  );
}

export function DropdownDivider() {
  return <div className="border-t border-white/10" />;
}
