# syntax=docker/dockerfile:1.7

FROM --platform=$TARGETPLATFORM rust:1-alpine AS builder

ARG TARGETPLATFORM

WORKDIR /build

RUN apk add --no-cache musl-dev gcc

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY tests/ ./tests/

RUN cargo build --release

FROM --platform=$TARGETPLATFORM alpine:3.23

RUN apk add --no-cache ffmpeg dcron curl

WORKDIR /app

COPY --from=builder /build/target/release/video-clip-extractor /usr/local/bin/video-clip-extractor

RUN chmod +x /usr/local/bin/video-clip-extractor

RUN mkdir -p /var/log

ENV VID_THEMER_VIDEO_DIR=""
ENV VID_THEMER_CRON_SCHEDULE="0 2 * * *"
ENV VID_THEMER_STRATEGY="random"
ENV VID_THEMER_RESOLUTION="1080p"
ENV VID_THEMER_AUDIO="true"
ENV VID_THEMER_CLIP_COUNT="2"
ENV VID_THEMER_INTRO_EXCLUSION="2.0"
ENV VID_THEMER_OUTRO_EXCLUSION="40.0"
ENV VID_THEMER_MIN_DURATION="20.0"
ENV VID_THEMER_MAX_DURATION="30.0"
ENV VID_THEMER_FORCE="false"
ENV VID_THEMER_HW_ACCEL="false"

RUN mkdir -p /etc/crontabs

COPY crontab /etc/crontabs/root

RUN chmod 0600 /etc/crontabs/root

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

CMD ["/bin/sh", "-c", "echo \"[$(date '+%Y-%m-%d %H:%M:%S %Z')] Container Vid-Themer started\"; echo \"[$(date '+%Y-%m-%d %H:%M:%S %Z')] Cron schedule: ${VID_THEMER_CRON_SCHEDULE}\"; echo \"[$(date '+%Y-%m-%d %H:%M:%S %Z')] Job log file: /var/log/video-clip-extractor.log\"; echo \"[$(date '+%Y-%m-%d %H:%M:%S %Z')] Starting cron daemon...\"; exec busybox crond -f -l 2 -L /dev/stdout"]
