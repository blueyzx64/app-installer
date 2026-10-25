<script lang="ts">
  /** Dashed arrow between the app and the folder. The dash animates (static when motion is reduced). */
</script>

<svg
  class="arrow"
  width="120"
  height="24"
  viewBox="0 0 120 24"
  aria-hidden="true"
  focusable="false"
>
  <path
    class="dash"
    d="M4 12H106"
    stroke="var(--glyph-muted)"
    stroke-width="2.5"
    stroke-dasharray="6 6"
    stroke-linecap="round"
  />
  <path
    d="M100 4l10 8-10 8"
    fill="none"
    stroke="var(--glyph-muted)"
    stroke-width="2.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>

<style>
  .dash {
    animation: var(--anim-dash);
  }
</style>
