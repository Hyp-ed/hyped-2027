<script>
    import LoginForm from "#lib/components/login-form.svelte";
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

<div class="flex min-h-svh flex-col items-center justify-center bg-muted p-6 md:p-10">
  <div class="w-full max-w-sm md:max-w-3xl">
    <LoginForm login={login} bind:email={email} bind:password={password}/>
  </div>
</div>
