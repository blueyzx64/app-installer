<script lang="ts">
  import { t } from '$lib/i18n';
  import FolderIcon from '$lib/ui/icons/FolderIcon.svelte';

  interface Props {
    /** The icon is held over the folder. */
    hover?: boolean;
    /** Play the "swallow" animation (after a drop). */
    gulp?: boolean;
    /** Label under the folder says "Update" for an app that is already installed. */
    isUpdate?: boolean;
    /** The tile element, for hit testing by the drag. */
    tile?: HTMLElement | null;
  }

  let { hover = false, gulp = false, isUpdate = false, tile = $bindable(null) }: Props = $props();
</script>

<div class="col">
  <div class="tile" class:hover class:gulp bind:this={tile} data-testid="folder-tile">
    <FolderIcon />
  </div>
  <span class="label">{isUpdate ? t('dmg.folderUpdate') : t('dmg.folder')}</span>
</div>

<style>
  .col {
    width: var(--col-dmg);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--gap-row);
  }
  .tile {
    width: var(--icon-dmg);
    height: var(--icon-dmg);
    border-radius: var(--r-folder);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: var(--tr-hover);
  }
  .tile.hover {
    background: var(--fill-hover);
    transform: scale(var(--hover-scale));
  }
  .tile.gulp {
    animation: var(--anim-gulp);
  }
  .label {
    font-size: var(--fs-headline);
    line-height: var(--lh-headline);
    font-weight: 600;
  }
</style>
