function getCookie(cname: string) {
  let name = cname + "=";
  let decodedCookie = decodeURIComponent(document.cookie);
  let ca = decodedCookie.split(';');
  for(let i = 0; i <ca.length; i++) {
    let c = ca[i];
    while (c.charAt(0) == ' ') {
      c = c.substring(1);
    }
    if (c.indexOf(name) == 0) {
      return c.substring(name.length, c.length);
    }
  }
  return "";
}
export async function loginFetcher(
	email: string,
	password: string
): Promise<{ message: string; success: boolean }> {
	const loginFetch = await fetch('/auth/login', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify({
			email: email,
			password: password
		})
	});
	if (!loginFetch.ok) {
		return { message: 'server error', success: false };
	}
	return loginFetch.json();
}

export async function protectedPing(): Promise<{ success: boolean }> {
    const ping = await fetch('/protected-api/ping', {
		method: 'GET',
		headers: {
			'Content-Type': 'application/json',
            'Authorization': "Bearer " +getCookie("AuthToken")
		}
    })
    if (ping.ok) {
        return {success: true}
    }
    return {success: false}
}
export async function whoami(): Promise<{success: true, name: string, email: string, created: string} | {success: false}> {
    const userFetch = await fetch('/auth/api/whoami', {
		method: 'GET',
		headers: {
			'Content-Type': 'application/json',
            'Authorization': "Bearer " +getCookie("AuthToken")
		}
    })
    if (userFetch.ok) {
    return await userFetch.json()
    }
    return {success: false}
}
