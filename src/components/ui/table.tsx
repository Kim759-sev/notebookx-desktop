import type { ComponentProps } from 'react';
import { cn } from './utils';

export function Table({ className, ...props }: ComponentProps<'table'>) {
  return <table data-slot="table" className={cn('w-full caption-bottom text-sm [&_th]:font-medium [&_tbody_tr]:transition-colors [&_tbody_tr:hover]:bg-muted/50', className)} {...props} />;
}
