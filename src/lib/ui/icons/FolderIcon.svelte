<script lang="ts">
  /** The Applications folder: accent-tinted folder with a 2x2 grid mark. */
  interface Props {
    width?: number;
  }
  let { width = 96 }: Props = $props();
</script>

<svg {width} height={(width * 80) / 96} viewBox="0 0 96 80" aria-hidden="true" focusable="false">
  <path d="M6 14a6 6 0 0 1 6-6h22l8 8h42a6 6 0 0 1 6 6v6H6z" fill="var(--folder-top)" />
  <rect x="6" y="22" width="84" height="52" rx="6" fill="var(--folder-body)" />
  <g fill="var(--folder-mark)" fill-opacity=".75">
    <rect x="34" y="34" width="12" height="12" rx="3" />
    <rect x="50" y="34" width="12" height="12" rx="3" />
    <rect x="34" y="50" width="12" height="12" rx="3" />
    <rect x="50" y="50" width="12" height="12" rx="3" />
  </g>
</svg>
