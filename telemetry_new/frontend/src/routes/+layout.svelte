<script lang="ts">
	import './layout.css';
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';
	import Hyped from '#lib/Hyped.svelte';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<div class="grid grid-rows-[auto_max-content]">
	<div class="grid grid-cols-[max-content_auto] items-center h-20">
		<a href="/" class="w-full pl-20">
			<Hyped />
		</a>
        <div class="pr-20 justify-self-end ">
        <a href="/login" class="outline-blue-800 rounded-full outline-2 p-2 bg-blue-300">Login</a>
        </div>
	</div>
	<div class="h-max">
		{@render children()}
	</div>
</div>
