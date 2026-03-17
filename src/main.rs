use clap::Parser;
use lspower::lsp::{FormattingOptions, FormattingProperty};
use satysfi_formatter::format;
use std::{fs, path::PathBuf};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// input file
    #[clap(parse(from_os_str), value_name = "FILE")]
    file: PathBuf,
    /// write to input file
    #[clap(short, long)]
    write: bool,
    /// output file
    #[clap(short, long)]
    output: Option<PathBuf>,
    /// indent size
    #[clap(short, long, default_value_t = 4)]
    indent_space: usize,
    /// preferred maximum line width
    #[clap(long, default_value_t = 120)]
    line_width: usize,
    /// Add space before arguments in command
    #[clap(long)]
    cspace: bool,
}

fn formatting_options_from_cli(cli: &Cli) -> FormattingOptions {
    FormattingOptions {
        tab_size: cli.indent_space as u32,
        properties: vec![(
            "lineWidth".to_string(),
            FormattingProperty::Number(cli.line_width.min(i32::MAX as usize) as i32),
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    }
}

fn main() {
    let cli = Cli::parse();
    let code = fs::read_to_string(&cli.file).expect("Failed to read file");
    let option = formatting_options_from_cli(&cli);
    let output = format(&code, option);

    match (cli.output, cli.write) {
        (Some(path), _) => fs::write(path, &output).expect("Failed to write file"),
        (None, true) => fs::write(&cli.file, &output).expect("Failed to write file"),
        (None, false) => println!("{}", output),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_line_width_option_sets_formatting_property() {
        let cli = Cli::parse_from(["satysfi-fmt", "input.saty", "--line-width", "140"]);
        let option = formatting_options_from_cli(&cli);

        assert_eq!(
            option.properties.get("lineWidth"),
            Some(&FormattingProperty::Number(140))
        );
    }
}
