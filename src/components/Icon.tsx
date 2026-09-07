import type { CSSProperties } from 'react';
import { Grid4 } from 'reicon-react/icons/Grid4';
import { Add } from 'reicon-react/icons/Add';
import { Refresh } from 'reicon-react/icons/Refresh';
import { ShieldTick } from 'reicon-react/icons/ShieldTick';
import { ArrowUpRight } from 'reicon-react/icons/ArrowUpRight';
import { Check } from 'reicon-react/icons/Check';
import { More } from 'reicon-react/icons/More';
import { Clock } from 'reicon-react/icons/Clock';
import { Sun2Newicons } from 'reicon-react/icons/Sun2Newicons';
import { Settings } from 'reicon-react/icons/Settings';
import { Trash } from 'reicon-react/icons/Trash';
import { Pin2 } from 'reicon-react/icons/Pin2';
import { Desktop } from 'reicon-react/icons/Desktop';
import { X } from 'reicon-react/icons/X';

const brandIcons = {
  terminal: new URL('../assets/codex.svg', import.meta.url).href,
  agent: new URL('../assets/pi-agent.svg', import.meta.url).href,
};

const icons = {
  grid: Grid4,
  plus: Add,
  refresh: Refresh,
  shield: ShieldTick,
  external: ArrowUpRight,
  check: Check,
  more: More,
  clock: Clock,
  sun: Sun2Newicons,
  settings: Settings,
  trash: Trash,
  pin: Pin2,
  desktop: Desktop,
  close: X,
};

type IconName = keyof typeof icons | keyof typeof brandIcons;

export function Icon({ name, size = 16, className, style }: { name: IconName; size?: number; className?: string; style?: CSSProperties }) {
  if (name === 'terminal' || name === 'agent') {
    const mask = `url("${brandIcons[name]}") center / contain no-repeat`;
    return <span aria-hidden="true" className={`brand-icon${className ? ` ${className}` : ''}`} style={{ width: size, height: size, display: 'inline-block', flexShrink: 0, backgroundColor: 'currentColor', WebkitMask: mask, mask, ...style }} />;
  }
  const Reicon = icons[name];
  return <Reicon aria-hidden="true" size={size} className={className} style={style} />;
}
