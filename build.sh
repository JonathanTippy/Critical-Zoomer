#!/bin/bash

set -e

lambdas=("leaderboard")

# Array of platforms to build for
#platforms=("linux/amd64" "linux/arm64")
platforms=("linux/arm64")

# you may need to run this first to ensure you can cross compile.
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes

# Iterate over each platform and run the cargo lambda build
for platform in "${platforms[@]}"; do
  # Replace "/" with "-" in the platform name for the Docker tag
  tag_name=$(echo "$platform" | sed 's/\//-/g')
  echo "Building for platform: $platform with tag: custom-cargo-lambda:$tag_name"
  # Build the image for each platform and load it locally
  docker buildx build -t custom-cargo-lambda:$tag_name -f Dockerfile.deploy . --platform="$platform" --load
done
docker images

declare -A target_map
target_map["linux/amd64"]="x86_64-unknown-linux-gnu"
target_map["linux/arm64"]="aarch64-unknown-linux-gnu"

for platform in "${platforms[@]}"; do

  # Replace "/" with "-" in the platform name for the Docker tag
  tag_name=$(echo "$platform" | sed 's/\//-/g')

  echo "Building for platform: $platform with tag: custom-cargo-lambda:$tag_name"
  target="${target_map[$platform]}"

  docker run -t --rm \
    --platform "$platform" \
    -v "$(pwd)":/app \
    --user "$(id -u):$(id -g)" \
    custom-cargo-lambda:$tag_name \
    --target="$target"
done
