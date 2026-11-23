import { Dialog as Base } from '@base-ui/react/dialog';
import { X } from 'lucide-react';
import type { ComponentProps, ReactNode } from 'react';
import { Button } from './button';
import { cn } from './utils';

export const Dialog = Base.Root;
export const DialogTitle = Base.Title;
export const DialogDescription = Base.Description;

export function DialogPopup({ children }: { children: ReactNode }) {
  return (
    <Base.Portal>
      <Base.Backdrop className="fixed inset-0 z-50 bg-black/40 backdrop-blur-xs" />
      <Base.Viewport className="pointer-events-none fixed inset-0 z-50 grid place-items-center p-3 sm:p-6">
        <Base.Popup data-slot="dialog-popup" className="pointer-events-auto relative max-h-full w-full max-w-xl overflow-y-auto overscroll-contain rounded-2xl border border-border bg-popover text-popover-foreground shadow-xl outline-none">
          {children}
        </Base.Popup>
      </Base.Viewport>
    </Base.Portal>
  );
}

export function DialogHeader({ children, closeLabel }: { children: ReactNode; closeLabel: string }) {
  return (
    <header className="relative space-y-1.5 px-6 pt-5 pr-16">
      {children}
      <Base.Close render={<Button variant="ghost" size="icon" />} className="absolute right-4 top-4" aria-label={closeLabel}><X /></Base.Close>
    </header>
  );
}

export function DialogPanel({ className, ...props }: ComponentProps<'div'>) {
  return <div className={cn('px-6 py-5', className)} {...props} />;
}

export function DialogFooter({ className, ...props }: ComponentProps<'div'>) {
  return <div className={cn('flex flex-wrap items-center justify-end gap-2 border-t border-border bg-muted/50 px-6 py-3.5', className)} {...props} />;
}
