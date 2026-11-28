use std::io;
use std::path::Path;

use blog::site::{self, BuildOptions, SiteConfig};

fn main() -> io::Result<()> {
    let config_path = Path::new("site.toml");
    let config = if config_path.is_file() {
        SiteConfig::from_file(config_path)?
    } else {
        SiteConfig::default()
    };
    let options = BuildOptions {
        config,
        ..BuildOptions::default()
    };

    let summary = site::build(&options)?;

    eprintln!(
        "Built {} post(s) into {}",
        summary.posts,
        options.output_dir.display()
    );
    Ok(())
}
