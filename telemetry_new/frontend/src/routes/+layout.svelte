<script lang="ts">
    import "./layout.css"
    import favicon from "#lib/assets/favicon.svg"
    import type { LayoutProps } from "./$types"
    import { ModeWatcher } from "mode-watcher"
    // import Hyped from '#lib/Hyped.svelte';
    // import { protectedPing } from '#lib/fetch/auth.ts';
    // import * as NavigationMenu from '#lib/components/ui/navigation-menu/index.js';
    import { page } from "$app/state"
    import Navbar from "#lib/normal_ui/navbar.svelte"
    let { children }: LayoutProps = $props()
    let isAtLoginPage = $derived(page.url.pathname === "/login")
</script>

<svelte:head>
    <link rel="icon" href={favicon} />
</svelte:head>
<ModeWatcher />

{#if !isAtLoginPage}
    <!-- <Hyped /> -->
    <div class="flex p-2">
        <Navbar />
    </div>
    <!-- <div class="justify-self-end pr-20"> -->
    <!-- 	{#await protectedPing()} -->
    <!-- 		<a href="/login" class="rounded-full bg-blue-300 p-2 outline-2 outline-blue-800">Login</a> -->
    <!-- 	{:then item} -->
    <!-- 		{#if item} -->
    <!-- 			<a href="/login" class="rounded-full bg-blue-300 p-2 outline-2 outline-blue-800">Login</a> -->
    <!-- 		{:else} -->
    <!-- 			<a href="/logout" class="rounded-full bg-blue-300 p-2 outline-2 outline-blue-800">Logout</a> -->
    <!-- 		{/if} -->
    <!-- 	{/await} -->
    <!-- </div> -->
    <!-- <div class="h-max"> -->
    <!-- </div> -->
{/if}

{@render children()}
