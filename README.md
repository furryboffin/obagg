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

Obagg is dependent on the Rust crate `tonic`, which in turn relies on the
`protoc` protobuf compiler. This can be installed locally:

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

You will probably want to run the grpc server and the client in different
terminals for simplicity.


## Configuration Options

In the `/conf` folder you will find an example configuration file: `obagg.yaml`.
to change the market you can edit the `ticker` value. The depth of the
aggregated orderbook is set by the `depth` value and the list of `exchanges`
that are used is also configurable, although this version of the server is only
designed to work with these two specific exchanges at this time. Future
adaptation of the server would aim to make it more generic and allow for several
more exchanges to be added. Lastly one can set `identical_level_order` to be
true or false depending on whether you want want identical levels to be ordered
with larger amounts towards or away from the middle of the book.

When different exchanges have identical levels in their books we must choose
the order. Setting this to true will order higher amounts closer to the center
of the orderbook. If using this aggregated orderbook to decide which exchange
to place orders for strategies such as arbitrage or market making, I would
set the paramter to be true, positioning larger liquidity closer to the centre
of the book. Moreover, speed is of the essence in such strategies and the fewer
levels we need to traverse to calculate predicted profit margins for specific
sized orders, the better.

## Future Improvements

The initial version is very rudimentary, much can be done to improve the server:

- Currently the exchange websocket consumers run in async function calls in the
main server thread. If one found that more resources were necessary to cope with
a larger number of client connections, one could spawn threads for them.

- If a websocket consumer returns an error the server will stop working. There
are several ways to address this issue. A simple approach would be to handle
returned errors and relaunch all the tasks if any of them fail. A better
approach might be to add a health monitor service that checks the health of the
consumers and the aggregator, relaunching them automatically in case they have
issues. It is also possible to have the tasks themselves manage their own state,
internally detecting any errors and relaunching internally when necessary. It
might be prudent to use a hybrid approach such that wherever simple the task
can manage it's own failures, but anything more terminal can be passed back as
an error which can then be handled by the main thread.

- parts of the code would benefit from being pulled out into helper functions
and more unit tests should be added for the various helper functions.

- Unit tests for the consumers can be added as well as the aggregator.

- Errors could be improved by using custom Error enums and converting all other
  errors into our custom type for top level handling.

- Binance and Bitstamp websocket message structures should be defined in custom
  structs. Error handling can then be moved into the struct implementation.

- Rather than defining the ticker in the conf file and restricting the server
  to serve only one ticker, allow the client to send a message to the gRPC
  server to select the ticker that they want. This would require adding inbound 
  message handling to the gRPC server implementation.

- To speed up the binance websocket client for depths 20 and under, one could
  select the other channel that provides entire orderbook messages rather than
  just updates. One could use the full solution for depths over 20.
