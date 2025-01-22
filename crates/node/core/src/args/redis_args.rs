//! Redis connection arguments

use clap::Parser;

/// Redis connection arguments
#[derive(Debug, Parser)]
pub struct RedisArgs {
    /// Redis URL for the database connection
    #[arg(long, value_name = "URL", default_value = "redis://localhost:6379")]
    pub redis_url: String,
}
