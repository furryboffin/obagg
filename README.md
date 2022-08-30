# Obagg (Orderbook Aggregator)

## Summary

Obagg is an Orderbook Aggregator gRPC server. This initial version aggregates
orderbook websockets streams from binance and bitstamp, but can be easily
extended to include other exchanges. Obagg handles errors in the websockets
consumers thus maintaining an open stream at all times. Note that the server is
designed such that if no clients are connected, the most recent incoming
orderbook messages are cached and the aggregated book created, while no
aggregated book is pushed out to client connection producers since there are
none to iterate over. 

Once at least one client is connected the aggregator will now have an up-to-date
cache and aggregated book for the very first output message pushed to the
first available client stream producer. Note also that only one instantiation
of the websocket consumers and aggregator are needed regardless of the number of
clients connected.

## Components

The application is separated in distinct components, each provided by a
dedicated executable file. Components are to be started independently as
required.

- `obagg grpc`: Start the Ordrebook Aggregator gRPC Stream Server.
- `obagg client`: Start a simple gRPC Client that connects to the gRPC Stream.
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
designed to work with these two specific exchanges at this time. The `exchanges`
config allows you to enable and disable each exchange as well as specify the
base URL for the websocket and API if required for snapshots. Future adaptation
of the server would aim to make it more generic and allow for several more
exchanges to be added. Lastly one can set `identical_level_order` to be true
or false depending on whether you want want identical levels to be ordered with
larger amounts towards or away from the middle of the book.

When different exchanges have identical levels in their books we must choose
the order. Setting this to true will order higher amounts closer to the center
of the orderbook. If using this aggregated orderbook to decide which exchange
to place orders for strategies such as arbitrage or market making, I would
set the paramter to be true, positioning larger liquidity closer to the centre
of the book. Moreover, speed is of the essence in such strategies and the fewer
levels we need to traverse to calculate predicted profit margins for specific
sized orders, the better.

To speed up the binance websocket client for depths 20 and under, obagg
selects the channel that provides entire orderbook messages rather than
just updates. Obagg uses the full orderbook snapshot and updates stream for
depths over 20.

## Future Improvements

There are still a few improvements that could be made to the server:

- Unit tests for the consumers can be added as well as the aggregator.

- Errors could be improved by using custom Error enums and converting all other
  errors into our custom type for top level handling.

- Rather than defining the ticker in the conf file and restricting the server
  to serve only one ticker, allow the client to send a message to the gRPC
  server to select the ticker that they want. This would require adding inbound 
  message handling to the gRPC server implementation.

- Add documentation for the gRPC server. One could use swagger to generate a
  served docuementation as an example.

- To alert connected clients of interruptions to the orderbook streams due to
  websocket failures, it would be prudent to add some form of quality control
  such that we guarantee the validity of any aggregated orderbooks. One simple
  way to achieve this would be to check for contiguity between messages from the
  websocket consumers' output streams. If a websocket dies and is relaunched,
  the subsequent event id arriving at the aggregator might not be contiguous,
  the aggregator could then hold off on pushing aggregated books until
  contiguity is restored.

- Currently for bitstamp, the depth of the book received is greater than the
  configured depth for the server. Consequently, some messages received include
  changes to the orderbook in levels outside the requested range. In this case
  the identical book is pushed to the aggregator. To reduce the number of
  duplicate messages being sent out of the aggregator, one should implement a
  comparrison check between the cached book and the newly received book, if
  there are no changes, there is no need to send out the aggregated book.
