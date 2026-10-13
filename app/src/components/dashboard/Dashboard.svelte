<script lang="ts">
  import ErrorBanner from './ErrorBanner.svelte';
  import InfoCards from './InfoCards.svelte';
  import Kpis from './Kpis.svelte';
  import NowPlayingCard from './NowPlayingCard.svelte';
  import QueueCard from './QueueCard.svelte';
  import SearchCard from './SearchCard.svelte';
  import SetupCard from './SetupCard.svelte';
</script>

<section class="tab" aria-label="Dashboard">
  <ErrorBanner />
  <SetupCard />
  <SearchCard />
  <Kpis />
  <div class="grid2"><NowPlayingCard /><QueueCard /></div>
  <InfoCards />
</section>
