# Changes to docker-related files

Docker image build and deployment settings for the oinori node.

---

## `docker/builder/build-node.sh`

### Change 1: Add the `CARGO_TARGET_DIR` environment variable
- **Change**: Added `CARGO_TARGET_DIR="target/${FEATURES:-"default"}"`
- **Purpose**: Separate the target directory when building with features enabled to avoid build cache collisions

### Change 2: Minor formatting fix
- **Change**: `else ` → `else` (removed trailing whitespace)

```diff
@@ -5,6 +5,7 @@ set -e

 PROFILE=${PROFILE:-release}
 FEATURES=${FEATURES:-""}
+CARGO_TARGET_DIR="target/${FEATURES:-"default"}"

 echo "Building aptos-node"
 echo "PROFILE: $PROFILE"
@@ -23,7 +24,7 @@ for PACKAGE in "${PACKAGES[@]}"; do
     if [ -n "$FEATURES" ] && [ "$PACKAGE" = "aptos-node" ]; then
         echo "Building aptos-node with features ${FEATURES}"
         cargo build --profile=$PROFILE --features=$FEATURES -p $PACKAGE "$@"
-    else
+    else
         # Build aptos-node separately
         cargo build --locked --profile=$PROFILE -p $PACKAGE "$@"
     fi
```

---

## `docker/builder/oinori.Dockerfile` (new)
- **Purpose**: Define a Docker image specifically for the oinori node
- **Structure (multi-stage build)**:
  1. **rust-base**: Base image with the standard build dependencies installed, including cmake, clang, git, openssl, libpq, and more
  2. **builder-base**: Install the protobuf compiler (`protoc`) and copy the source tree
  3. **aptos-node-builder**: Build the aptos-node binary using `docker/builder/build-node.sh`
  4. **validator (final image)**: Runtime image that copies in the built binary and oinori-specific configuration

- **oinori-specific settings**:
  - Copy the Move contracts (`oinori/oinori-mev/contracts/`) to `/opt/oinori/contracts/`
  - Copy `genesis.blob`, `waypoint.txt`, and `fullnode.yaml`
  - Set the `OINORI_CONTRACT_DIR` environment variable

```dockerfile
# oinori-node-specific part (excerpt)

### Run Validator Image
FROM rust-base AS validator
RUN ln -sf /usr/bin/perf_* /usr/bin/perf
RUN addgroup --system --gid 6180 aptos && adduser --system --ingroup aptos --no-create-home --uid 6180 aptos

RUN mkdir -p /opt/aptos/etc
COPY --link --from=aptos-node-builder /aptos/dist/aptos-node /usr/local/bin/

## --- oinori ------------------------------------------------------------------
COPY --link ./oinori/oinori-mev/contracts/ /opt/oinori/contracts/

COPY --link ./oinori-configs/mainnet-config/genesis.blob /opt/aptos/genesis/genesis.blob
COPY --link ./oinori-configs/mainnet-config/waypoint.txt /opt/aptos/genesis/waypoint.txt
COPY --link ./oinori-configs/mainnet-config/fullnode.yaml /opt/aptos/fullnode.yaml

ENV OINORI_CONTRACT_DIR /opt/oinori/contracts/
```

---

## `docker/oinori-build-rust-all.sh` (new)
- **Purpose**: Rust build script for the oinori node
- **Content**: Builds only the aptos-node package and copies it into `dist/`
- **Difference from the original script**: The original `docker-bake-rust-all.sh` builds many binaries (tools, faucet, and more), while the oinori version limits the build to aptos-node only to reduce build time

```bash
#!/bin/bash
set -e

PROFILE=${PROFILE:-release}
FEATURES=${FEATURES:-""}

echo "Building aptos-node"
echo "PROFILE: $PROFILE"
echo "FEATURES: $FEATURES"
echo "CARGO_TARGET_DIR: $CARGO_TARGET_DIR"

if [ -n "$FEATURES" ]; then
    echo "Building aptos-node with features ${FEATURES}"
    cargo build --profile=$PROFILE --features=$FEATURES -p aptos-node "$@"
else
    cargo build --locked --profile=$PROFILE \
        -p aptos-node \
        "$@"
fi

BINS=(
    aptos-node
)

mkdir dist

for BIN in "${BINS[@]}"; do
    cp target/$PROFILE/$BIN dist/$BIN
done
```
