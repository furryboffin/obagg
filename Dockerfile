# stage 1 - generate recipe file for dependencies
FROM rust as planner
WORKDIR /app
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# stage 2 - build our dependencies
FROM rust as cacher
WORKDIR /app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# stage 3 
# use the main official rust docker image
FROM rust as builder

# copy the app into the docker image
COPY . /app

# set the working directory
WORKDIR /app
ENV PROTOC=/usr/bin/protoc

# build the app
# RUN cargo install tonic
RUN apt update && apt upgrade -y
RUN apt install -y protobuf-compiler libprotobuf-dev

COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# build the app
RUN cargo build --release

FROM busybox:1.35.0-uclibc as busybox

# use google distroless as runtime image
FROM gcr.io/distroless/cc-debian11 as grpc

# Now copy the static shell into base image.
COPY --from=busybox /bin/sh /bin/sh

COPY --from=builder /app/target/release/obagg /app/
COPY --from=builder /app/conf/obagg.yaml /app/obagg.yaml
COPY --from=builder /app/scripts/start.sh /app/start.sh
WORKDIR /app
ENV AGGREGATED_ORDERBOOK_CONFIG="/app/obagg.yaml"

# start the gRPC server and run it in the background
CMD ["/bin/sh", "./start.sh"]

