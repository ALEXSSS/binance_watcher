use clap::Parser;
use std::fmt;

/// Help page configuration struct
#[derive(Parser, Default)]
#[command(version)]
pub struct Config {
    /// number of active connections to binance allowed to open, can be less, if instruments.len < connections
    #[arg(short, long, default_value_t = 1, value_parser=clap::value_parser!(u32).range(1..200))]
    pub connections: u32,

    /// number of levels to display
    #[arg(short, long, default_value_t = 20, value_parser=clap::value_parser!(u32).range(1..200))]
    pub levels: u32,

    /// delay between updates displayed in ms (not supported)
    #[arg(short, long, default_value_t = 1000, value_parser=clap::value_parser!(u32).range(1..2000000))]
    pub delay: u32,

    /// instruments to watch
    #[arg(short, long, default_values_t = ["btcusdt".to_string()])]
    pub instruments: Vec<String>,

    /// websocket binance url
    #[arg(long, default_value = "wss://fstream.binance.com/ws")]
    pub ws_api_url: String,

    /// api binance url
    #[arg(long, default_value = " https://fapi.binance.com/fapi/v1")]
    pub api_url: String,
}

impl Config {
    /// calculates number of instruments per connection
    pub fn instruments_per_connection(&self) -> usize {
        (self.instruments.len() as f32 / self.connections as f32).ceil() as usize
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "====START PARAMETERS====")?;
        writeln!(f, "binance url: {}", self.ws_api_url)?;
        writeln!(f, "instruments: [{}]", self.instruments.join(","))?;
        writeln!(f, "book's levels to display: {}", self.levels)?;
        writeln!(f, "screen update interval ms: {}", self.delay)?;
        writeln!(f, "binance connections pool size: {}", self.connections)?;
        writeln!(f, "====END PARAMETERS====")?;
        Ok(())
    }
}

mod test {
    use crate::console_arguments::Config;

    #[test]
    fn test_instruments_per_connection() {
        let mut config: Config = Default::default();
        config.connections = 2;
        config.instruments = vec![
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
        ];

        assert_eq!(config.instruments_per_connection(), 3)
    }
}
