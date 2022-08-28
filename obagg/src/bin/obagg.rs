mod log;
use ::log::error;
use structopt::StructOpt;

#[derive(StructOpt)]
struct OBAggregatorOpt {
    #[structopt(long = "--no-syslog")]
    disable_syslog: bool,
    #[structopt(subcommand)]
    subcommand: Subcommand,
}

#[derive(StructOpt)]
enum Subcommand {
    Grpc,
    Client,
    Version,
}

#[tokio::main]
async fn main() {
    let opt = OBAggregatorOpt::from_args();
    match opt.subcommand {
        Subcommand::Grpc => {
            log::init("obagg-server".to_string(), opt.disable_syslog);
            if let Err(e) = obagg::server(obagg::config::read_config()).await {
                error!("Error returned from Server : {}", e);
            }
        }
        Subcommand::Client => {
            log::init("obagg-client".to_string(), opt.disable_syslog);
            if let Err(e) = obagg::client(obagg::config::read_config()).await {
                error!("Error returned from Server : {}", e);
            }
        }
        Subcommand::Version => {
            println!("Obagg Orderbook Aggregator {}", env!("CARGO_PKG_VERSION"));
        }
    }
}
