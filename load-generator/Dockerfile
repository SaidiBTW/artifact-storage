FROM docker.io/library/python:3.14.5-slim-bookworm@sha256:a9bee15510a364124aa24692899d269835683b883de42f7ebec8c293cf679ccb AS base

FROM base AS builder
RUN apt-get -qq update \
  && apt-get install -y --no-install-recommends \
  g++ \
  libffi-dev \
  && rm -rf /var/lib/apt/lists/*

COPY ./load-generator/requirements.txt .
RUN pip install --prefix="/reqs" -r requirements.txt

FROM base
WORKDIR /usr/src/app/
COPY --from=builder /reqs /usr/local
COPY ./load-generator/locustfile.py .
ENTRYPOINT [ "locust", "--skip-log-setup" ]