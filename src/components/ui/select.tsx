import { Select as SelectPrimitive } from '@base-ui/react/select';
import { Check, ChevronDown } from 'lucide-react';
import { Children, isValidElement, type ChangeEvent, type ReactNode } from 'react';
import { cn } from './utils';

interface OptionEntry {
  value: string;
  label: ReactNode;
  text: string;
  disabled?: boolean;
}

function collectOptions(nodes: ReactNode, out: OptionEntry[] = []): OptionEntry[] {
  Children.forEach(nodes, (child) => {
    if (!isValidElement<{ value?: string | number; disabled?: boolean; children?: ReactNode }>(child)) {
      return;
    }
    if (child.type === 'option') {
      const text = Children.toArray(child.props.children)
        .map((n) => (typeof n === 'string' || typeof n === 'number' ? String(n) : ''))
        .join('')
        .trim();
      const rawVal = child.props.value !== undefined ? String(child.props.value) : text;
      out.push({
        value: rawVal,
        label: child.props.children,
        text: text || rawVal,
        disabled: child.props.disabled,
      });
    } else if (child.props?.children) {
      collectOptions(child.props.children, out);
    }
  });
  return out;
}

export interface SelectProps {
  value?: string | number;
  defaultValue?: string | number;
  onChange?: (event: ChangeEvent<HTMLSelectElement>) => void;
  disabled?: boolean;
  required?: boolean;
  name?: string;
  id?: string;
  'aria-label'?: string;
  className?: string;
  size?: 'sm' | 'default' | 'lg';
  children?: ReactNode;
}

export function Select({
  value,
  defaultValue,
  onChange,
  disabled = false,
  required = false,
  name,
  id,
  'aria-label': ariaLabel,
  className,
  size = 'default',
  children,
}: SelectProps) {
  const options = collectOptions(children);
  const currentValue =
    value !== undefined
      ? String(value)
      : defaultValue !== undefined
        ? String(defaultValue)
        : options[0]?.value ?? '';
  const selectedOption = options.find((o) => o.value === currentValue) ?? options[0];

  function handleValueChange(nextVal: string | null) {
    if (nextVal === null || !onChange) return;
    const synthetic = {
      target: { value: nextVal, name: name ?? '' },
      currentTarget: { value: nextVal, name: name ?? '' },
    } as unknown as ChangeEvent<HTMLSelectElement>;
    onChange(synthetic);
  }

  return (
    <SelectPrimitive.Root
      value={currentValue}
      onValueChange={handleValueChange}
      disabled={disabled || options.length === 0}
      required={required}
      name={name}
    >
      <SelectPrimitive.Trigger
        id={id}
        aria-label={ariaLabel}
        data-slot="select-trigger"
        className={cn(
          'relative inline-flex w-full min-w-32 cursor-pointer items-center justify-between gap-2 rounded-lg border border-input bg-background px-3 text-sm text-foreground shadow-xs/5 outline-none transition-shadow select-none before:pointer-events-none before:absolute before:inset-0 before:rounded-[calc(var(--radius-lg)-1px)] not-disabled:not-focus-visible:before:shadow-[0_1px_rgba(0,0,0,0.04)] focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/24 disabled:pointer-events-none disabled:opacity-50 dark:bg-input/32 dark:not-disabled:not-focus-visible:before:shadow-[0_-1px_rgba(255,255,255,0.06)]',
          size === 'sm' ? 'h-7.5 px-2.5 text-xs' : size === 'lg' ? 'h-9.5' : 'h-8.5',
          className,
        )}
      >
        <span className="truncate text-left font-normal">
          {selectedOption ? selectedOption.label : <SelectPrimitive.Value />}
        </span>
        <SelectPrimitive.Icon className="shrink-0 text-muted-foreground">
          <ChevronDown className="size-3.5 opacity-75" />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal>
        <SelectPrimitive.Positioner sideOffset={6} alignItemWithTrigger={false} className="z-[120]">
          <SelectPrimitive.Popup
            data-slot="select-popup"
            className="max-h-72 min-w-[max(var(--anchor-width),10rem)] overflow-y-auto rounded-xl border border-border bg-popover p-1 text-popover-foreground shadow-xl ring-1 ring-black/5 outline-none dark:ring-white/10"
          >
            <SelectPrimitive.List className="flex flex-col gap-0.5">
              {options.map((opt) => (
                <SelectPrimitive.Item
                  key={opt.value}
                  value={opt.value}
                  disabled={opt.disabled}
                  className="relative flex cursor-pointer items-center justify-between gap-3 rounded-lg px-2.5 py-1.5 text-sm text-popover-foreground outline-none select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-45 data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
                >
                  <SelectPrimitive.ItemText className="truncate">{opt.label}</SelectPrimitive.ItemText>
                  <SelectPrimitive.ItemIndicator className="inline-flex shrink-0 items-center text-foreground">
                    <Check className="size-3.5" />
                  </SelectPrimitive.ItemIndicator>
                </SelectPrimitive.Item>
              ))}
            </SelectPrimitive.List>
          </SelectPrimitive.Popup>
        </SelectPrimitive.Positioner>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
