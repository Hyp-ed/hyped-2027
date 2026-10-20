# Telemetry

## Current Structure of this folder

- `backend` contains the backend using go fiber

- `frontend` contains the front end which would be design by the marketing team and make with SvelteKit

- `swagger` contains the config file of swagger

## How to run

> [!IMPORTANT]
> Install docker beforehand

To run the telemetry backend, frontend and swagger docs

`docker compose up -d`

To stop all of them:

`docker compose down`

### Backend

The Backend is hosted on `localhost:8000` in the testing environment

To get live build log messages of the backend do `docker compose logs air -f`

> [!TIP]
> The backend automatically hot reload and rebuilt the application, if you are in the test environment

### Swagger

To navigate to the swagger UI (the API docs) go to `localhost:8080`

## New backend plans

- [x] implement swagger docs into docker compose
- [ ] implement swagger docs - authentication
- [ ] implement swagger docs - public app
- [ ] implement swagger docs - state transition
- [ ] implement all the API end point written by the docs

## The API directory

`/` - public data page
`/auth/login` & `/auth/signup`
`/dashboard` will be protected
`/protected-api/`

## Swagger Docs

Swagger are located on `localhost:8080`

Swagger docs are written in `swagger/swagger.yaml`
