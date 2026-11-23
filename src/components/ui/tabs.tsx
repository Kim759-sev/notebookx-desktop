import { Tabs as Base } from '@base-ui/react/tabs';

export const Tabs = Base.Root;
export const TabsPanel = Base.Panel;
export function TabsList(props: Base.List.Props) {
  return <Base.List className="inline-flex max-w-full gap-1 overflow-x-auto rounded-lg border border-border bg-muted p-1" {...props} />;
}
export function TabsTrigger(props: Base.Tab.Props) {
  return <Base.Tab className="shrink-0 cursor-pointer whitespace-nowrap rounded-md border border-transparent px-3 py-1 text-sm text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring data-active:border-border data-active:bg-popover data-active:text-foreground data-active:shadow-xs/5" {...props} />;
}
