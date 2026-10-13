<script lang="ts">
  import { ICONS, type IconName } from '../lib/icons';

  let { name }: { name: IconName } = $props();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true">{@html ICONS[name]}</svg>
