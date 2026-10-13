# Telemetry

## Current Structure of this folder

- `backend` contains the backend using go fiber

- `frontend` contains the front end which would be design by the marketing team and make with SvelteKit

- `swagger` contains the config file of swagger

## How to run

To run the telemetry backend, frontend and swagger docs

`docker compose up -d`

**TO DO, THE BACKEND AND FRONTEND NOT COMPLETE**

## New backend plans

- [x] implement swagger docs into docker compose
- [ ] implement swagger docs - authentication
- [ ] implement swagger docs - public app
- [ ] implement swagger docs - state transition
- [ ] implement swagger docs - state subscription
- [ ] implement all the API end point written by the docs

## The API directory

`/` - public data page
`/auth/login` & `/auth/signup`
`/dashboard` will be protected

## Swagger Docs

Swagger are located on `localhost:8080`

Swagger docs are written in `swagger/swagger.yaml`
