<script lang="ts">
  /** Generic package icon, tinted with the accent. Shown when a package has no icon of its own. */
  interface Props {
    size?: number;
  }
  let { size = 112 }: Props = $props();
</script>

<svg width={size} height={size} viewBox="0 0 112 112" aria-hidden="true" focusable="false">
  <rect x="4" y="4" width="104" height="104" rx="24" fill="var(--accent)" />
  <rect x="30" y="24" width="52" height="64" rx="6" fill="var(--app-paper)" />
  <path
    d="M40 42h32M40 52h32M40 62h20"
    stroke="var(--accent)"
    stroke-width="4"
    stroke-linecap="round"
  />
  <circle cx="75" cy="77" r="10" fill="var(--app-badge)" />
</svg>
