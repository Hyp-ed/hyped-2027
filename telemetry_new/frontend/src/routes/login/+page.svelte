<script>
	import { loginFetcher } from "#lib/fetch/auth.ts";
    import {goto} from "$app/navigation"

	let email = $state('');
	let password = $state('');

	async function login() {
		if (email == '' || password == '') {
            alert("Empty email or password")
            return
		}
        let fetch = await loginFetcher(email, password)
        if (!fetch.success) {
            alert("Server Errror. Reason: " + fetch.message)
            return
        }
        goto("/dashboard")
        return
	}
</script>

<div class="flex h-[calc(100vh-5rem)] items-center justify-center">
	<div class="grid grid-cols-1 items-center justify-center justify-items-center gap-2">
		<div class="underline">Sign In</div>
		<input type="email" bind:value={email} placeholder="email" />
		<input type="password" bind:value={password} placeholder="password" />

		<button
			class="w-full cursor-pointer rounded-full bg-blue-300 p-2 outline-2 outline-blue-800"
			onclick={login}>Login</button
		>
	</div>
</div>
