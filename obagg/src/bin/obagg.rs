mod log;
use ::log::error;
use serde::de::DeserializeOwned;
// use serde::Deserialize;
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
            if let Err(e) = obagg::server(read_config()).await {
                error!("Error returned from Server : {}", e);
                println!("Error returned from Server : {}", e);
            }
        }
        Subcommand::Client => {
            log::init("obagg-client".to_string(), opt.disable_syslog);
            if let Err(e) = obagg::client(read_config()).await {
                error!("Error returned from Server : {}", e);
                println!("Error returned from Server : {}", e);
            }
        }
        Subcommand::Version => {
            println!("OBAggregator {}", env!("CARGO_PKG_VERSION"));
        }
    }
}

fn file_from_env(var: &str) -> Result<std::fs::File, Box<dyn std::error::Error + Send + Sync>> {
    let path = std::env::var(var)?;
    Ok(std::fs::File::open(path)?)
}

fn read_config<T: DeserializeOwned>() -> T {
    let file = file_from_env("AGGREGATED_ORDERBOOK_CONFIG")
        .expect("Error when opening file pointed to by AGGREGATED_ORDERBOOK_CONFIG env variable");
    serde_yaml::from_reader(file).expect("Error parsing configuration file")
}
