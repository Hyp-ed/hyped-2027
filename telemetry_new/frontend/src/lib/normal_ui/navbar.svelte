<script lang="ts">
	import * as NavigationMenu from "#lib/components/ui/navigation-menu/index.js";
	import { navigationMenuTriggerStyle } from "#lib/components/ui/navigation-menu/navigation-menu-trigger.svelte";
 
</script>
<NavigationMenu.Root>
	<NavigationMenu.List>
		<NavigationMenu.Item>
			<NavigationMenu.Link href="/login" class={navigationMenuTriggerStyle()}>
                Login
			</NavigationMenu.Link>
		</NavigationMenu.Item>
        <NavigationMenu.Item>
        </NavigationMenu.Item>
	</NavigationMenu.List>
</NavigationMenu.Root>
