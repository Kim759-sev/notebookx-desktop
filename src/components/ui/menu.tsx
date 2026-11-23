import { Menu as MenuPrimitive } from '@base-ui/react/menu';
import { Check } from 'lucide-react';
import type { ReactNode } from 'react';
import { cn } from './utils';

export interface MenuOption<T extends string> {
  value: T;
  label: ReactNode;
  icon?: ReactNode;
}

export function ChoiceMenu<T extends string>({
  value,
  options,
  onChange,
  disabled,
  ariaLabel,
  trigger,
  align = 'end',
}: {
  value: T;
  options: MenuOption<T>[];
  onChange: (next: T) => void;
  disabled?: boolean;
  ariaLabel?: string;
  trigger: ReactNode;
  align?: 'start' | 'center' | 'end';
}) {
  return (
    <MenuPrimitive.Root disabled={disabled}>
      <MenuPrimitive.Trigger
        aria-label={ariaLabel}
        className={cn(
          'relative inline-flex h-8 shrink-0 cursor-pointer items-center justify-center gap-1.5 rounded-lg border border-input bg-background px-2.5 text-xs font-medium text-foreground shadow-xs/5 outline-none transition-colors select-none hover:bg-accent/60 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/24 disabled:pointer-events-none disabled:opacity-50 dark:bg-input/32',
        )}
      >
        {trigger}
      </MenuPrimitive.Trigger>
      <MenuPrimitive.Portal>
        <MenuPrimitive.Positioner sideOffset={6} align={align} className="z-[120]">
          <MenuPrimitive.Popup className="min-w-36 rounded-xl border border-border bg-popover p-1 text-popover-foreground shadow-xl ring-1 ring-black/5 outline-none dark:ring-white/10">
            <MenuPrimitive.RadioGroup value={value} onValueChange={(v) => onChange(v as T)}>
              {options.map((opt) => (
                <MenuPrimitive.RadioItem
                  key={opt.value}
                  value={opt.value}
                  closeOnClick
                  className="flex cursor-pointer items-center justify-between gap-3 rounded-lg px-2.5 py-1.5 text-sm text-popover-foreground outline-none select-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
                >
                  <span className="inline-flex items-center gap-2">
                    {opt.icon}
                    <span>{opt.label}</span>
                  </span>
                  <MenuPrimitive.RadioItemIndicator className="inline-flex items-center text-foreground">
                    <Check className="size-3.5" />
                  </MenuPrimitive.RadioItemIndicator>
                </MenuPrimitive.RadioItem>
              ))}
            </MenuPrimitive.RadioGroup>
          </MenuPrimitive.Popup>
        </MenuPrimitive.Positioner>
      </MenuPrimitive.Portal>
    </MenuPrimitive.Root>
  );
}
