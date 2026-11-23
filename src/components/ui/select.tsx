import type { ComponentProps } from 'react';
import { cn } from './utils';

export function Select({ className, ...props }: ComponentProps<'select'>) {
  return <select className={cn('min-w-0 max-w-full cursor-pointer rounded-lg border border-input bg-background px-2.5 py-1.5 text-sm text-foreground shadow-xs/5 outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/24 disabled:opacity-50 dark:bg-input/32', className)} {...props} />;
}
