import Tag from '@arco-design/web-react/es/Tag';

export function CurrentAccountBadge({ name }: { name: 'Codex' | 'Pi Agent' }) {
  const runtime = name === 'Codex' ? 'codex' : 'pi';
  return <Tag className={`current-badge ${runtime}`} size="small" bordered={false}>{name} 当前账号</Tag>;
}
