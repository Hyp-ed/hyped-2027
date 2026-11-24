<script lang="ts">
    import { whoami } from "#lib/fetch/auth.ts"
    import { onMount } from "svelte"
    let name: string = $state("")
    onMount(async () => {
        let userdata = await whoami()
        if (userdata.success == true) {
            name = userdata.name
        }
    })
</script>

<div class="flex h-[calc(100vh-5rem)] p-5">
    <div>Dashboard:</div>
    <div>Welcome back {name}</div>
</div>
