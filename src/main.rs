#[macro_use] extern crate error_chain; // https://stevedonovan.github.io/rust-gentle-intro/6-error-handling.html#error-chain-for-serious-errors
#[macro_use] extern crate clap; // https://docs.rs/clap/2.32.0/clap/
#[macro_use] extern crate nom; // https://stevedonovan.github.io/rust-gentle-intro/nom-intro.html
extern crate yaml_rust; // http://chyh1990.github.io/yaml-rust/

use std::fs::File;
use std::io::Read;
use std::io::BufReader;
use yaml_rust::Yaml;

mod errors { error_chain!{} }
use errors::*;

// https://stackoverflow.com/questions/46876879/how-do-i-create-a-streaming-parser-in-nom
// https://mustache.github.io/mustache.5.html

#[derive(Debug, PartialEq)]
pub enum Token {
	Literal(String),
	DirectSub(Vec<String>),
	CondSub(Vec<String>),
	InvSub(Vec<String>),
	EndSub,
	KeySub(i64),
	Comment(String),
}

named!(yaml_path<&str, Vec<String>>,
	do_parse!(
		path: ws!(pair!(opt!(char!('.')), separated_list!(char!('.'), ws!(nom::alpha)))) >>
		({ let mut ret = path.1.iter().map(|x| x.to_string()).collect::<Vec<String>>(); if path.0.is_some() { ret.insert(0, "".to_string()); ret } else { ret } })
	)
);

named!(template_sub<&str, Token>,
	delimited!(
		tag_s!("{{"),
		switch!(opt!(one_of!("#/^!?")),
			None => do_parse!(path: yaml_path >> (Token::DirectSub(path))) |
			Some('#') => do_parse!(path: yaml_path >> (Token::CondSub(path))) |
			Some('^') => do_parse!(path: yaml_path >> (Token::InvSub(path))) |
			Some('/') => do_parse!((Token::EndSub)) | // FIXME How do I return Token::EndSub without this pointless do_parse?
			Some('?') => do_parse!(n: ws!(opt!(nom::digit)) >> (Token::KeySub(n.map(|x| x.parse::<i64>().expect("Not an integer")).unwrap_or(0)))) | // TODO I think I can do the conversion automatically with parse_to
			Some('!') => do_parse!(text: take_until!("}}") >> (Token::Comment(text.to_string())))
		),
		tag_s!("}}")
	)
);

named!(template_literal<&str, Token>,
	do_parse!(
		content: alt!(take_until!("{{") | nom::rest_s) >> // FIXME How do I ensure here that ALL input is processed?
		(Token::Literal(content.to_string()))
	)
);

named!(document<&str, Vec<Token>>,
	many0!(
		alt!(complete!(template_sub) | complete!(template_literal))
	)
);

fn yaml_get<'a>(yaml: &'a Yaml, context: &Vec<Vec<&str>>, path: &Vec<&str>) -> &'a Yaml { // TODO For these functions, should I be using &[&str], &Vec<&str>, &Vec<String>, ...?
	let mut cur = yaml;
	for elem in context.iter().flat_map(|x| x.iter()).chain(path.iter()) {
		cur = if *elem == "" { yaml } else { &cur[*elem] }
	}
	cur
}

fn yaml_string(yaml: &Yaml) -> Result<String> {
	match yaml {
		Yaml::Real(x) => Ok(x.to_string()),
		Yaml::Integer(x) => Ok(x.to_string()),
		Yaml::String(x) => Ok(x.to_string()),
		Yaml::Boolean(x) => Ok(x.to_string()),
		Yaml::Null => Ok("".to_string()),
		_ => bail!("Can't stringify type"),
	}
}

fn run() -> Result<()> {
	let args = clap_app!(tpl =>
		(about: "Simple multi-purpose template engine")
		(@arg input: * index(1) "File to be templated")
		(@arg values: * index(2) "YAML file of template values")
	).get_matches();

	let mut input = String::new();
	BufReader::new(File::open(args.value_of("input").unwrap()).chain_err(|| "Failed to open input file")?)
		.read_to_string(&mut input).chain_err(|| "Failed to read from input file")?;
	let template = document(&input).expect("Failed to parse template"); // FIXME Why doesn't the borrow checker like this? .chain_err(|| "Failed to parse template")?;
	let mut tokens = template.1;
	tokens.push(Token::Literal(template.0.to_string())); // FIXME This hack will go away when I figure out how to make Nom parse all input
	//println!("{:?}", tokens);

	let mut yamlin = String::new();
	BufReader::new(File::open(args.value_of("values").unwrap()).chain_err(|| "Failed to open values file")?)
		.read_to_string(&mut yamlin).chain_err(|| "Failed to read from values file")?;
	let values = &yaml_rust::YamlLoader::load_from_str(&yamlin).chain_err(|| "Failed to parse values file")?[0]; // TODO What should we do if there are multiple streams in the file?  Ignore them?

	let mut context: Vec<Vec<String>> = vec![];
	let mut echo_enable = vec![];

	for token in tokens {
		//println!("\x1b[31m{:?} {:?} {:?}\x1b[m", token, context, echo_enable);
		let out: Result<String> = match token {
			Token::Literal(s) => Ok(s),
			Token::Comment(_) => Ok("".to_string()),
			Token::DirectSub(ref path) => yaml_string(yaml_get(values, &context.iter().map(|x| x.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>(), &path.iter().map(AsRef::as_ref).collect())), // TODO Abstract the ugly map to a function or something to minimize repeated code
			Token::CondSub(ref path) => {
				echo_enable.push(!yaml_get(values, &context.iter().map(|x| x.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>(), &path.iter().map(AsRef::as_ref).collect()).is_badvalue());
				context.push(path.to_vec()); // TODO Can I do it without the redundan conversion?
				Ok("".to_string())
			},
			Token::InvSub(ref path) => {
				echo_enable.push(yaml_get(values, &context.iter().map(|x| x.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>(), &path.iter().map(AsRef::as_ref).collect()).is_badvalue());
				context.push(path.to_vec());
				Ok("".to_string())
			},
			Token::EndSub => { echo_enable.pop(); context.pop(); Ok("".to_string()) },
			Token::KeySub(n) => Ok(n.to_string()), // TODO context.iter().rev().nth(n as usize).map(|x| x.to_string()).ok_or(Error::from("Key level too high")),
		};
		print!("{}", if echo_enable.iter().all(|x| *x) { out } else { Ok("".to_string()) }.chain_err(|| "Failed to template value")?);
	}
	Ok(())
}

quick_main!(run);
