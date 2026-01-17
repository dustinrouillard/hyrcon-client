FROM gcr.io/distroless/static:nonroot
LABEL org.opencontainers.image.source=https://github.com/dustinrouillard/hyrcon-client

ENV PATH=/

COPY artifacts/hyrcon-client /hyrcon-client

USER nonroot:nonroot
CMD ["hyrcon-client"]
