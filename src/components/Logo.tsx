/** The Justice "J" mark, drawn in the current accent colour. */
export default function Logo({ size = 20 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden="true">
      <rect x="32" y="32" width="960" height="960" rx="200" fill="var(--bg-2)" />
      <rect x="52" y="52" width="920" height="920" rx="182" fill="none" stroke="var(--red-2)" strokeWidth="40" />
      <path d="M300 232H760V352H300Z" fill="var(--red)" />
      <path d="M560 232H760V600C760 750 660 824 520 824C400 824 308 764 284 654L464 610C474 654 500 672 528 672C548 672 560 660 560 630Z" fill="var(--red)" />
    </svg>
  );
}
