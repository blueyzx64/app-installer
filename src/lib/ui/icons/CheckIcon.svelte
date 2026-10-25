<script lang="ts">
  /** Check mark stroke. Inherits `currentColor`. */
  interface Props {
    size?: number;
  }
  let { size = 34 }: Props = $props();
</script>

<svg width={size} height={size} viewBox="0 0 36 36" aria-hidden="true" focusable="false">
  <path
    d="M9 18.5l6 6 12-13"
    fill="none"
    stroke="currentColor"
    stroke-width="3.6"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>
