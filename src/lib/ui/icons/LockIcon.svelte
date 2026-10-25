<script lang="ts">
  /** Padlock glyph. Always sits next to text, never alone. Inherits `currentColor`. */
  interface Props {
    size?: number;
  }
  let { size = 12 }: Props = $props();
</script>

<svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true" focusable="false">
  <rect
    x="2"
    y="5.5"
    width="8"
    height="5.5"
    rx="1.2"
    fill="none"
    stroke="currentColor"
    stroke-width="1.3"
  />
  <path d="M4 5.5V4a2 2 0 0 1 4 0v1.5" fill="none" stroke="currentColor" stroke-width="1.3" />
</svg>
