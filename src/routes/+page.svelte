<script lang="ts">
  // Router: launch file given -> installer, otherwise the library.
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { useControllers } from '$lib/app/context';

  const { installer } = useControllers();

  $effect(() => {
    let cancelled = false;
    void installer.takeLaunch().then((hasFile) => {
      if (!cancelled) void goto(resolve(hasFile ? '/install' : '/library'), { replaceState: true });
    });
    return () => {
      cancelled = true;
    };
  });
</script>
