FROM node:26-alpine AS builder

WORKDIR /app/

COPY package*.json ./
RUN npm install
COPY . .

RUN npm run build

CMD ["echo", "finish building"]
