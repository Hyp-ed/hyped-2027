# Telemetry

## Current Structure of this folder

- `backend` contains the backend using go fiber

- `frontend` contains the front end which would be make with SvelteKit

- `swagger` contains the config file of swagger

- `database` contains some essential database scripts

## How to run

> [!IMPORTANT]
> Install docker beforehand

To run the development environment that includes:

- Backend (Air - Golang will automatically refresh)
- Frontend (Rebuilt by default if you restart the container with `--build` flag)
- Swagger Docs
- PostgresDB (Store authentication)
- Adminer (The database management software for **DEBUGGING ONLY**)

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

### Database

The database have it's entry point from `database/entry_point_script`

### Frontend

The frontend will be rebuilt when you start the docker compose.

> [!IMPORTANT]
> To update the front end you must rebuilt the docker compose.
> Stop the current docker compose
> `docker compose down -v`
> Then restart and build the docker compose
> `docker compose up -d --force-recreate --build`

## Swagger Docs

Swagger are located on `localhost:8080`

Swagger docs are written in `swagger/swagger.yaml`
