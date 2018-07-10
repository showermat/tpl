#[macro_use] extern crate error_chain; // https://stevedonovan.github.io/rust-gentle-intro/6-error-handling.html#error-chain-for-serious-errors
#[macro_use] extern crate clap; // https://docs.rs/clap/2.32.0/clap/
#[macro_use] extern crate nom; // https://stevedonovan.github.io/rust-gentle-intro/nom-intro.html
extern crate yaml_rust; // http://chyh1990.github.io/yaml-rust/

use std::fs::File;
use std::io::Read;
use std::io::BufReader;

mod errors { error_chain!{} }
use errors::*;

// https://stackoverflow.com/questions/46876879/how-do-i-create-a-streaming-parser-in-nom
// https://mustache.github.io/mustache.5.html

#[derive(Debug, PartialEq)]
pub enum Segment {
	Literal(String),
	Sub(Option<char>, String)
}

named!(template_sub<&str, Segment>,
	do_parse!(
		tag_s!("{{") >>
		sigil: opt!(ws!(one_of!("#/^!"))) >>
		content: ws!(nom::alpha) >>
		tag_s!("}}") >>
		(Segment::Sub(sigil, content.to_string()))
	)
);

named!(template_literal<&str, Segment>,
	do_parse!(
		content: take_until!("{{") >>
		(Segment::Literal(content.to_string()))
	)
);

named!(document<&str, Vec<Segment>>,
	many0!(
		alt!(complete!(template_sub) | complete!(template_literal))
	)
);

fn run() -> Result<()> {
	let args = clap_app!(tpl =>
		(about: "Simple multi-purpose template engine")
		(@arg input: * index(1) "File to be templated")
	).get_matches();
	let mut f = BufReader::new(File::open(args.value_of("input").unwrap()).chain_err(|| "Failed to open input file")?);
	let mut content = String::new();
	f.read_to_string(&mut content).chain_err(|| "Failed to read from input file")?;
	println!("{:?}", document(&content));
	Ok(())
}

quick_main!(run);
