import Tag from '@arco-design/web-react/es/Tag';
import { Icon } from './Icon';
export function CurrentAccountBadge({ name }: { name: string }) {
  return <Tag className="current-badge" color="arcoblue" size="small" bordered={false} icon={<Icon name="check" size={12}/>}>{name} 当前账号</Tag>;
}
