import { Icon } from './Icon';
export function CurrentAccountBadge({ name }: { name: string }) {
  return <span className="badge"><Icon name="check" size={12}/>{name} 当前账号</span>;
}
