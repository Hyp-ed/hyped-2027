# Multi-stage build
# Step 1: Front end building 
FROM node:lts AS nodebuild

WORKDIR /nodebuild/

# Download Dependencies
COPY frontend/package*.json ./
RUN npm install

# Build!
COPY frontend .
RUN npm run build

# Step 2: Backend binary building 
FROM golang:1.27.0 AS gobinary

WORKDIR /gobinary

# Download Dependencies
COPY backend/go.mod backend/go.sum ./
RUN go mod download

# Build!
COPY backend .
COPY --from=nodebuild /nodebuild/build ./frontend_build

RUN CGO_ENABLED=0 GOOS=linux go build -o /binary

FROM scratch
COPY --from=gobinary /binary /binary
EXPOSE 8000/tcp

ENV POSTGRES_HOST=db
ENV POSTGRES_USER=user
ENV POSTGRES_PASSWORD=example
ENV POSTGRES_DB=user

CMD ["/binary"]
