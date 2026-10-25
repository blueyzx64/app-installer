<script lang="ts">
  /** Small spinning indicator (NSProgressIndicator, spinning style). Pulses when motion is reduced. */
  interface Props {
    size?: number;
    /** Accessible name. Omit when a nearby label already says what is happening. */
    label?: string;
  }
  let { size = 12, label }: Props = $props();
</script>

<span
  class="spinner"
  style:width="{size}px"
  style:height="{size}px"
  role={label ? 'status' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
></span>

<style>
  .spinner {
    display: inline-block;
    flex: none;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    opacity: 0.85;
    animation: var(--anim-spin);
  }
</style>
