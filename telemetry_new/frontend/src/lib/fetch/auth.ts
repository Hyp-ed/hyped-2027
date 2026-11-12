export async function loginFetcher(email: string, password: string): Promise<{message: string, success: boolean}> {
	const loginFetch = await fetch('/auth/login', {
		method: 'POST',
		body: JSON.stringify({
			email: email,
			password: password
		})
	});
    if (!loginFetch.ok) {
        return {"message": "server error", "success": false}
    }
    return loginFetch.json()
}
