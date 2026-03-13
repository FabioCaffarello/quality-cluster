# syntax=docker/dockerfile:1.7

ARG GO_VERSION=1.25.7
ARG ALPINE_VERSION=3.20

FROM golang:${GO_VERSION}-alpine AS builder

WORKDIR /src

ARG SERVICE
ARG TARGETOS=linux
ARG TARGETARCH=amd64

RUN test -n "${SERVICE}"

RUN apk add --no-cache ca-certificates

COPY . .

RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
    go build -trimpath -ldflags="-s -w" -o /out/service ./cmd/${SERVICE} \
    && test -x /out/service

FROM alpine:${ALPINE_VERSION}

RUN apk add --no-cache ca-certificates \
    && addgroup -S app \
    && adduser -S -G app app \
    && mkdir -p /etc/quality-service \
    && chown -R app:app /etc/quality-service

COPY --from=builder /out/service /usr/local/bin/service

USER app:app
ENTRYPOINT ["/usr/local/bin/service"]
