import type { ComponentProps } from 'react';
import { cn } from './utils';

export function Card({ className, ...props }: ComponentProps<'section'>) {
  return <section data-slot="card" className={cn('min-w-0 rounded-xl border border-border bg-card p-5 text-card-foreground shadow-xs/5', className)} {...props} />;
}
