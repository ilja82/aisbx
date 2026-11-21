# syntax=docker/dockerfile:1
FROM scratch
COPY --chmod=0755 files/ /usr/local/lib/aisbx/

# See kits/statusline/statusline.dockerfile for why, and who reads it.
ENV SBX_KIT_HOST_GRADLE_PROPERTIES=1
