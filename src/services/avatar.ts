import type { CSSProperties } from 'react';

export function accountAvatarStyle(name: string): CSSProperties {
  // Stable across sessions and re-imports: the normalized account name owns its color.
  let hash = 0;
  for (const char of name.trim().toLowerCase()) hash = (Math.imul(hash, 31) + char.charCodeAt(0)) | 0;
  const hue = (hash >>> 0) % 360;
  return {
    backgroundColor: `hsl(${hue} 42% 93%)`,
    color: `hsl(${hue} 34% 45%)`,
    borderColor: `hsl(${hue} 34% 87%)`,
  };
}
