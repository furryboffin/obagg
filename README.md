# Obagg (Orderbook Aggregator)

## Components

The application is separated in distinct components, each provided by a
dedicated executable file. Nothing is provided to start all the components.
Components are to be started independently as required.

- `obagg grpc`: Start the Ordrebook Aggregator gRPC Stream Server.
- `obagg client`: Start a gRPC Client that connects to the gRPC Stream.
- `obagg version`: Get the version of obagg.


## Docker Container

For testing purposes a very simple `start.sh` script is included in the
`/scripts` folder. This script is launched inside a docker container as an
example to show the gRPC server and client running together in tandem.

## Running Obagg in Docker

Once you have cloned the github repository locally, cd into the project folder:

```cd obagg```

Then run the following commands to build and run the docker container:

```
docker build -t obagg .
docker run -t obagg
```

## Running Obagg locally

Obagg is dependent on the Rust crate `tonic`, which in turn relies on the `protoc` protobuf compiler. This can be installed locally:

```
apt update && apt upgrade -y
apt install -y protobuf-compiler libprotobuf-dev
```

Once you have cloned the github repository locally, cd into the project folder:

```cd obagg```

Now you can compile and run locally with the following commands:
```
cargo build
export AGGREGATED_ORDERBOOK_CONFIG="conf/obagg.yaml"
cargo run --bin obagg -- --no-syslog grpc
cargo run --bin obagg -- --no-syslog client
```

You will probably want to run the grpc server and the client in different terminals for simplicity.


## Configuration Options

In the `/conf` folder you will find an example configuration file: `obagg.yaml`.
to change the market you can edit the `ticker` value. The depth of the aggregated orderbook is set by the `depth` value and the list of `websockets` that are used is also configurable, although this version of the server is only designed to work with these two specific exchanges at this time. Future adaptation of the server would aim to make it more generic and allow for several more exchanges to be added.
