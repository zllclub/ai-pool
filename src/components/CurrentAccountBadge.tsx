export function CurrentAccountBadge({ name }: { name: string }) {
  return <span className="badge"><span className="status-dot" />{name} ✓</span>;
}
